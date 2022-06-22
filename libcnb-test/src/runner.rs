use crate::{
    app, build, util, BuildpackReference, PackBuildCommand, PullPolicy, TestConfig, TestContext,
};
use bollard::Docker;
use std::env;
use std::env::VarError;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Runner for libcnb integration tests.
///
/// # Dependencies
/// Integration tests require external tools to be available on the host to run:
/// - [pack](https://buildpacks.io/docs/tools/pack/)
/// - [Docker](https://www.docker.com/)
///
/// # Example
/// ```no_run
/// use libcnb_test::{TestConfig, BuildpackReference, assert_contains, TestRunner};
///
/// # fn call_test_fixture_service(addr: std::net::SocketAddr, payload: &str) -> Result<String, ()> {
/// #    unimplemented!()
/// # }
/// TestRunner::default().run_test(
///     TestConfig::new("heroku/builder:22", "test-fixtures/app").buildpacks(vec![
///         BuildpackReference::Other(String::from("heroku/openjdk")),
///         BuildpackReference::Crate,
///     ]),
///     |context| {
///         assert_contains!(context.pack_stdout, "---> Maven Buildpack");
///         assert_contains!(context.pack_stdout, "---> Installing Maven");
///         assert_contains!(context.pack_stdout, "---> Running mvn package");
///
///         context
///             .prepare_container()
///             .expose_port(12345)
///             .start_with_default_process(|container| {
///                 assert_eq!(
///                     call_test_fixture_service(
///                         container.address_for_port(12345).unwrap(),
///                         "Hagbard Celine"
///                     )
///                     .unwrap(),
///                     "enileC drabgaH"
///                 );
///             });
///     },
/// );
/// ```
pub struct TestRunner {
    pub(crate) docker: Docker,
    pub(crate) tokio_runtime: tokio::runtime::Runtime,
}

impl Default for TestRunner {
    fn default() -> Self {
        let tokio_runtime =
            tokio::runtime::Runtime::new().expect("Could not create internal Tokio runtime");

        let docker = match env::var("DOCKER_HOST") {
            #[cfg(target_family = "unix")]
            Ok(docker_host) if docker_host.starts_with("unix://") => {
                Docker::connect_with_unix_defaults()
            }
            Ok(docker_host)
            if docker_host.starts_with("tcp://") || docker_host.starts_with("https://") =>
                {
                    #[cfg(not(feature = "remote-docker"))]
                    panic!("Cannot connect to DOCKER_HOST '{docker_host}' since it requires TLS. Please use a local Docker daemon instead (recommended), or else enable the experimental `remote-docker` feature.");
                    #[cfg(feature = "remote-docker")]
                    Docker::connect_with_ssl_defaults()
                }
            Ok(docker_host) => panic!("Cannot connect to unsupported DOCKER_HOST '{docker_host}'"),
            Err(VarError::NotPresent) => Docker::connect_with_local_defaults(),
            Err(VarError::NotUnicode(_)) => {
                panic!("DOCKER_HOST environment variable is not unicode encoded!")
            }
        }
            .expect("Could not connect to local Docker daemon");

        TestRunner::new(tokio_runtime, docker)
    }
}

impl TestRunner {
    /// Creates a new runner that uses the given Tokio runtime and Docker connection.
    ///
    /// This function is meant for advanced use-cases where fine control over the Tokio runtime
    /// and/or Docker connection is required. For the common use-cases, use `Runner::default`.
    pub fn new(tokio_runtime: tokio::runtime::Runtime, docker: Docker) -> Self {
        TestRunner {
            docker,
            tokio_runtime,
        }
    }

    /// Starts a new integration test run.
    ///
    /// This function will copy the application to a temporary directory, cross-compiles this crate,
    /// packages it as a buildpack and then invokes [pack](https://buildpacks.io/docs/tools/pack/)
    /// to build a new Docker image with the buildpacks specified by the passed [`TestConfig`].
    ///
    /// Since this function is supposed to only be used in integration tests, failures are not
    /// signalled via [`Result`](Result) values. Instead, this function panics whenever an unexpected error
    /// occurred to simplify testing code.
    ///
    /// # Panics
    /// - When the app could not be copied
    /// - When this crate could not be packaged as a buildpack
    /// - When the `pack` command unexpectedly fails
    ///
    /// # Example
    /// ```no_run
    /// use libcnb_test::{assert_contains, TestRunner, TestConfig};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/app"),
    ///     |context| {
    ///         assert_contains!(context.pack_stdout, "---> Ruby Buildpack");
    ///         assert_contains!(context.pack_stdout, "---> Installing bundler");
    ///         assert_contains!(context.pack_stdout, "---> Installing gems");
    ///     },
    /// )
    /// ```
    pub fn run_test<C: Into<TestConfig>, F: FnOnce(TestContext)>(&self, config: C, f: F) {
        self.run_test_internal(util::random_docker_identifier(), config, f);
    }

    pub(crate) fn run_test_internal<C: Into<TestConfig>, F: FnOnce(TestContext)>(
        &self,
        image_name: String,
        config: C,
        f: F,
    ) {
        let config = config.into();

        let app_dir = if config.app_dir.is_relative() {
            env::var("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .expect("Could not determine Cargo manifest directory")
                .join(&config.app_dir)
        } else {
            config.app_dir.clone()
        };

        let temp_app_dir =
            app::copy_app(&app_dir).expect("Could not copy app to temporary location");

        if let Some(app_dir_preprocessor) = &config.app_dir_preprocessor {
            (app_dir_preprocessor)(PathBuf::from(temp_app_dir.path()));
        }

        let temp_crate_buildpack_dir = if config.buildpacks.contains(&BuildpackReference::Crate) {
            Some(
                build::package_crate_buildpack(&config.target_triple)
                    .expect("Could not package current crate as buildpack"),
            )
        } else {
            None
        };

        let mut pack_command = PackBuildCommand::new(
            &config.builder_name,
            temp_app_dir.path(),
            &image_name,
            // Prevent redundant image-pulling, which slows tests and risks hitting registry rate limits.
            PullPolicy::IfNotPresent,
        );

        config.env.iter().for_each(|(key, value)| {
            pack_command.env(key, value);
        });

        for buildpack in &config.buildpacks {
            match buildpack {
                BuildpackReference::Crate => {
                    pack_command.buildpack(temp_crate_buildpack_dir.as_ref()
                        .expect("Test references crate buildpack, but crate wasn't packaged as a buildpack. This is an internal libcnb-test error, please report any occurrences."))
                }
                BuildpackReference::Other(id) => pack_command.buildpack(id.clone()),
            };
        }

        let output = Command::from(pack_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Could not spawn external 'pack' process")
            .wait_with_output()
            .expect("Error while waiting on external 'pack' process");

        let test_context = TestContext {
            pack_stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            pack_stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            app_dir: PathBuf::from(temp_app_dir.path()),
            image_name,
            config,
            runner: self,
        };

        if output.status.success() {
            f(test_context);
        } else {
            panic!(
                "pack command failed with exit-code {}!\n\npack stdout:\n{}\n\npack stderr:\n{}",
                output
                    .status
                    .code()
                    .map_or(String::from("<unknown>"), |exit_code| exit_code.to_string()),
                test_context.pack_stdout,
                test_context.pack_stderr
            )
        }
    }
}
