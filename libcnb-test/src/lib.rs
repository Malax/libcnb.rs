// Enable rustc and Clippy lints that are disabled by default.
// https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#unused-crate-dependencies
#![warn(unused_crate_dependencies)]
// https://rust-lang.github.io/rust-clippy/stable/index.html
#![warn(clippy::pedantic)]
// This lint is too noisy and enforces a style that reduces readability in many cases.
#![allow(clippy::module_name_repetitions)]

mod app;
mod build;
mod container_context;
mod container_port_mapping;
mod log;
mod macros;
mod pack;
mod runner;
mod util;

pub use crate::container_context::{ContainerContext, PrepareContainerContext};
use crate::pack::{PackBuildCommand, PullPolicy};
pub use crate::runner::TestRunner;
use bollard::image::RemoveImageOptions;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Configuration for a test.
#[derive(Clone)]
pub struct TestConfig {
    app_dir: PathBuf,
    target_triple: String,
    builder_name: String,
    buildpacks: Vec<BuildpackReference>,
    env: HashMap<String, String>,
    app_dir_preprocessor: Option<Rc<dyn Fn(PathBuf)>>,
}

// When builder functions are used on TestConfig, the return value is &mut TestConfig
impl From<&mut TestConfig> for TestConfig {
    fn from(c: &mut TestConfig) -> Self {
        (*c).clone()
    }
}

/// References a Cloud Native Buildpack
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BuildpackReference {
    /// References the buildpack in the Rust Crate currently being tested
    Crate,
    /// References another buildpack by id, local directory or tarball
    Other(String),
}

impl TestConfig {
    /// Creates a new test configuration.
    ///
    /// If the `app_dir` parameter is a relative path, it is treated as relative to the Cargo
    /// manifest directory ([`CARGO_MANIFEST_DIR`](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates)),
    /// i.e. the package's root directory.
    pub fn new(builder_name: impl Into<String>, app_dir: impl AsRef<Path>) -> Self {
        TestConfig {
            app_dir: PathBuf::from(app_dir.as_ref()),
            target_triple: String::from("x86_64-unknown-linux-musl"),
            builder_name: builder_name.into(),
            buildpacks: vec![BuildpackReference::Crate],
            env: HashMap::new(),
            app_dir_preprocessor: None,
        }
    }

    /// Sets the buildpacks order.
    ///
    /// Defaults to [`BuildpackReference::Crate`].
    pub fn buildpacks(&mut self, buildpacks: impl Into<Vec<BuildpackReference>>) -> &mut Self {
        self.buildpacks = buildpacks.into();
        self
    }

    /// Sets the target triple.
    ///
    /// Defaults to `x86_64-unknown-linux-musl`.
    pub fn target_triple(&mut self, target_triple: impl Into<String>) -> &mut Self {
        self.target_triple = target_triple.into();
        self
    }

    /// Inserts or updates an environment variable mapping for the build process.
    ///
    /// Note: This does not set this environment variable for running containers, it's only
    /// available during the build.
    ///
    /// # Example
    /// ```no_run
    /// use libcnb_test::{TestConfig, TestRunner};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/app")
    ///         .env("ENV_VAR_ONE", "VALUE ONE")
    ///         .env("ENV_VAR_TWO", "SOME OTHER VALUE"),
    ///     |context| {
    ///         // ...
    ///     },
    /// )
    /// ```
    pub fn env(&mut self, k: impl Into<String>, v: impl Into<String>) -> &mut Self {
        self.env.insert(k.into(), v.into());
        self
    }

    /// Adds or updates multiple environment variable mappings for the build process.
    ///
    /// Note: This does not set environment variables for running containers, they're only
    /// available during the build.
    ///
    /// # Example
    /// ```no_run
    /// use libcnb_test::{TestConfig, TestRunner};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/app").envs(vec![
    ///         ("ENV_VAR_ONE", "VALUE ONE"),
    ///         ("ENV_VAR_TWO", "SOME OTHER VALUE"),
    ///     ]),
    ///     |context| {
    ///         // ...
    ///     },
    /// );
    /// ```
    pub fn envs<K: Into<String>, V: Into<String>, I: IntoIterator<Item = (K, V)>>(
        &mut self,
        envs: I,
    ) -> &mut Self {
        envs.into_iter().for_each(|(key, value)| {
            self.env(key.into(), value.into());
        });

        self
    }

    /// Sets an app directory preprocessor function.
    ///
    /// It will be run after the app directory has been copied for the current integration test run,
    /// the changes will not affect other integration test runs.
    ///
    /// Generally, we suggest using dedicated test fixtures. However, in some cases it is more
    /// economical to slightly modify a fixture programmatically before a test instead.
    ///
    /// # Example
    /// ```no_run
    /// use libcnb_test::{TestConfig, TestRunner};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/app").app_dir_preprocessor(
    ///         |app_dir| std::fs::remove_file(app_dir.join("Procfile")).unwrap(),
    ///     ),
    ///     |context| {
    ///         // ...
    ///     },
    /// );
    /// ```
    pub fn app_dir_preprocessor<F: 'static + Fn(PathBuf)>(&mut self, f: F) -> &mut Self {
        self.app_dir_preprocessor = Some(Rc::new(f));
        self
    }
}

/// Context for a currently executing test.
pub struct TestContext<'a> {
    /// Standard output of `pack`, interpreted as an UTF-8 string.
    pub pack_stdout: String,
    /// Standard error of `pack`, interpreted as an UTF-8 string.
    pub pack_stderr: String,
    /// The directory of the app this integration test uses.
    ///
    /// This is a copy of the `app_dir` directory passed to [`TestConfig::new`] and unique to
    /// this integration test run. It is safe to modify the directory contents inside the test.
    pub app_dir: PathBuf,
    /// The configuration used for this integration test.
    pub config: TestConfig,

    image_name: String,
    runner: &'a TestRunner,
}

impl<'a> TestContext<'a> {
    /// Prepares a new container with the image from the test.
    ///
    /// This will not create nor run the container immediately. Use the returned
    /// `PrepareContainerContext` to configure the container, then call
    /// [`start_with_default_process`](PrepareContainerContext::start_with_default_process) on it
    /// to actually create and start the container.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use libcnb_test::{TestConfig, TestRunner};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/empty-app"),
    ///     |context| {
    ///         context
    ///             .prepare_container()
    ///             .start_with_default_process(|container| {
    ///                 // ...
    ///             });
    ///     },
    /// );
    /// ```
    #[must_use]
    pub fn prepare_container(&self) -> PrepareContainerContext {
        PrepareContainerContext::new(self)
    }

    /// Starts a subsequent integration test run.
    ///
    /// This function behaves exactly like [`TestRunner::run_test`], but it will reuse the OCI image
    /// from the previous test, causing the CNB lifecycle to restore cached layers. It will use the
    /// same [`TestRunner`] as the previous test run.
    ///
    /// This function allows testing of subsequent builds, including caching logic and buildpack
    /// behaviour when build environment variables change, stacks are upgraded and more.
    ///
    /// Note that this function will consume the current context. This is because the image will
    /// be changed by the subsequent test, invalidating the context. Running a subsequent test must
    /// therefore be the last operation. You can nest subsequent runs if required.
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
    pub fn run_test<C: Into<TestConfig>, F: FnOnce(TestContext)>(self, config: C, f: F) {
        self.runner
            .run_test_internal(self.image_name.clone(), config, f);
    }

    /// Starts a subsequent integration test run with inherited configuration.
    ///
    /// This function is the same as [`TestContext::run_test`] but automatically inherits the
    /// configuration from the previous test run. See [`TestContext::run_test`] for details.
    ///
    /// # Panics
    /// - When the app could not be copied
    /// - When this crate could not be packaged as a buildpack
    /// - When the `pack` command unexpectedly fails
    pub fn run_test_inherit_config<F: FnOnce(TestContext)>(self, f: F) {
        self.runner
            .run_test_internal(self.image_name.clone(), self.config.clone(), f);
    }
}

impl<'a> Drop for TestContext<'a> {
    fn drop(&mut self) {
        // We do not care if image removal succeeded or not. Panicking here would result in
        // SIGILL since this function might be called in a Tokio runtime.
        let _image_delete_result =
            self.runner
                .tokio_runtime
                .block_on(self.runner.docker.remove_image(
                    &self.image_name,
                    Some(RemoveImageOptions {
                        force: true,
                        ..RemoveImageOptions::default()
                    }),
                    None,
                ));
    }
}

// This runs the README.md as a doctest, ensuring the code examples in it are valid.
// It will not be part of the final crate.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
