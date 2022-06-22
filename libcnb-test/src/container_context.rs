use crate::log::LogOutput;
use crate::{container_port_mapping, util};
use crate::{log, TestContext};
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PrepareContainerContext<'a> {
    test_context: &'a TestContext<'a>,
    exposed_ports: Vec<u16>,
    env: HashMap<String, String>,
}

impl<'a> PrepareContainerContext<'a> {
    pub(crate) fn new(test_context: &'a TestContext) -> Self {
        Self {
            test_context,
            exposed_ports: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Exposes a given port of the container to the host machine.
    ///
    /// The given port is mapped to a random port on the host machine. Use
    /// [`ContainerContext::address_for_port`] to obtain the local port for a mapped port.
    pub fn expose_port(&mut self, port: u16) -> &mut Self {
        self.exposed_ports.push(port);
        self
    }

    /// Inserts or updates an environment variable mapping for the container.
    ///
    /// # Example
    /// ```no_run
    /// use libcnb_test::{TestConfig, TestRunner};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/app"),
    ///     |context| {
    ///         context
    ///             .prepare_container()
    ///             .envs(vec![("FOO", "FOO_VALUE"), ("BAR", "BAR_VALUE")])
    ///             .start_with_default_process(|container| {
    ///                 // ...
    ///             })
    ///     },
    /// );
    /// ```
    pub fn env(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Adds or updates multiple environment variable mappings for the container.
    ///
    /// # Example
    /// ```no_run
    /// use libcnb_test::{TestConfig, TestRunner};
    ///
    /// TestRunner::default().run_test(
    ///     TestConfig::new("heroku/builder:22", "test-fixtures/app"),
    ///     |context| {
    ///         context
    ///             .prepare_container()
    ///             .envs(vec![("FOO", "FOO_VALUE"), ("BAR", "BAR_VALUE")])
    ///             .start_with_default_process(|container| {
    ///                 // ...
    ///             })
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

    /// Creates and starts the container configured by this context using the image's default
    /// CNB process.
    ///
    /// See: [CNB App Developer Guide: Run a multi-process app - Default process type](https://buildpacks.io/docs/app-developer-guide/run-an-app/#default-process-type)
    ///
    /// # Panics
    /// - When the container could not be created
    /// - When the container could not be started
    pub fn start_with_default_process<F: FnOnce(ContainerContext)>(&self, f: F) {
        self.start_internal(None, None, f);
    }

    /// Creates and starts the container configured by this context using the image's default
    /// CNB process and given arguments.
    ///
    /// See: [CNB App Developer Guide: Run a multi-process app - Default process type with additional arguments](https://buildpacks.io/docs/app-developer-guide/run-an-app/#default-process-type-with-additional-arguments)
    ///
    /// # Panics
    /// - When the container could not be created
    /// - When the container could not be started
    pub fn start_with_default_process_args<
        F: FnOnce(ContainerContext),
        A: IntoIterator<Item = I>,
        I: Into<String>,
    >(
        &self,
        args: A,
        f: F,
    ) {
        self.start_internal(None, Some(args.into_iter().map(I::into).collect()), f);
    }

    /// Creates and starts the container configured by this context using the given CNB process.
    ///
    /// See: [CNB App Developer Guide: Run a multi-process app - Non-default process-type](https://buildpacks.io/docs/app-developer-guide/run-an-app/#non-default-process-type)
    ///
    /// # Panics
    /// - When the container could not be created
    /// - When the container could not be started
    pub fn start_with_process<F: FnOnce(ContainerContext), P: Into<String>>(
        &self,
        process: P,
        f: F,
    ) {
        self.start_internal(Some(vec![process.into()]), None, f);
    }

    /// Creates and starts the container configured by this context using the given CNB process
    /// and arguments.
    ///
    /// See: [CNB App Developer Guide: Run a multi-process app - Non-default process-type with additional arguments](https://buildpacks.io/docs/app-developer-guide/run-an-app/#non-default-process-type-with-additional-arguments)
    ///
    /// # Panics
    /// - When the container could not be created
    /// - When the container could not be started
    pub fn start_with_process_args<
        F: FnOnce(ContainerContext),
        A: IntoIterator<Item = I>,
        I: Into<String>,
        P: Into<String>,
    >(
        &self,
        process: P,
        args: A,
        f: F,
    ) {
        self.start_internal(
            Some(vec![process.into()]),
            Some(args.into_iter().map(I::into).collect()),
            f,
        );
    }

    /// Creates and starts the container configured by this context using the given shell command.
    ///
    /// The CNB lifecycle launcher will be implicitly used. Environment variables will be set. Uses
    /// `/bin/sh` as the shell.
    ///
    /// See: [CNB App Developer Guide: Run a multi-process app - User-provided shell process](https://buildpacks.io/docs/app-developer-guide/run-an-app/#user-provided-shell-process)
    ///
    /// # Panics
    /// - When the container could not be created
    /// - When the container could not be started
    pub fn start_with_shell_command<F: FnOnce(ContainerContext), C: Into<String>>(
        &self,
        command: C,
        f: F,
    ) {
        self.start_internal(
            Some(vec![String::from(CNB_LAUNCHER_PATH)]),
            Some(vec![
                String::from(SHELL_PATH),
                String::from("-c"),
                command.into(),
            ]),
            f,
        );
    }

    fn start_internal<F: FnOnce(ContainerContext)>(
        &self,
        entrypoint: Option<Vec<String>>,
        cmd: Option<Vec<String>>,
        f: F,
    ) {
        let container_name = util::random_docker_identifier();

        self.test_context.runner.tokio_runtime.block_on(async {
            self.test_context
                .runner
                .docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: container_name.clone(),
                    }),
                    Config {
                        image: Some(self.test_context.image_name.clone()),
                        env: Some(self.env.iter().map(|(k, v)| format!("{k}={v}")).collect()),
                        entrypoint,
                        cmd,
                        ..container_port_mapping::port_mapped_container_config(&self.exposed_ports)
                    },
                )
                .await
                .expect("Could not create container");

            self.test_context
                .runner
                .docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await
                .expect("Could not start container");
        });

        f(ContainerContext {
            container_name,
            test_context: self.test_context,
        });
    }
}

pub struct ContainerContext<'a> {
    pub container_name: String,
    pub(crate) test_context: &'a TestContext<'a>,
}

impl<'a> ContainerContext<'a> {
    /// Gets the container's log output until the current point in time.
    ///
    /// Note: This method will only return logs until the current point in time. It will not
    /// block until the container stops. Since the output of this method depends on timing, directly
    /// asserting on its contents might result in flaky tests.
    ///
    /// See: [`logs_wait`](Self::logs_wait) for a blocking alternative.
    ///
    /// # Panics
    /// - When the log output could not be consumed/read.
    #[must_use]
    pub fn logs_now(&self) -> LogOutput {
        // Bollard forces us to cast to i64
        #[allow(clippy::cast_possible_wrap)]
        self.logs_internal(bollard::container::LogsOptions {
            stdout: true,
            stderr: true,
            since: 0,
            until: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_secs() as i64,
            tail: "all",
            ..bollard::container::LogsOptions::default()
        })
    }

    /// Gets the container's log output until the container stops.
    ///
    /// Note: This method will block until the container stops. If the container never stops by
    /// itself, your test will hang indefinitely. This is common when the container hosts an HTTP
    /// service.
    ///
    /// See: [`logs_now`](Self::logs_now) for a non-blocking alternative.
    ///
    /// # Panics
    /// - When the log output could not be consumed/read.
    #[must_use]
    pub fn logs_wait(&self) -> LogOutput {
        self.logs_internal(bollard::container::LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            tail: "all",
            ..bollard::container::LogsOptions::default()
        })
    }

    #[must_use]
    fn logs_internal<T: Into<String> + Serialize>(
        &self,
        logs_options: bollard::container::LogsOptions<T>,
    ) -> LogOutput {
        self.test_context
            .runner
            .tokio_runtime
            .block_on(log::consume_container_log_output(
                self.test_context
                    .runner
                    .docker
                    .logs(&self.container_name, Some(logs_options)),
            ))
            .expect("Could not consume container log output")
    }

    /// # Panics
    #[must_use]
    pub fn address_for_port(&self, port: u16) -> Option<SocketAddr> {
        self.test_context.runner.tokio_runtime.block_on(async {
            self.test_context
                .runner
                .docker
                .inspect_container(&self.container_name, None)
                .await
                .unwrap()
                .network_settings
                .and_then(|network_settings| network_settings.ports)
                .and_then(|ports| {
                    container_port_mapping::parse_port_map(&ports)
                        .unwrap()
                        .get(&port)
                        .copied()
                })
        })
    }

    /// Executes a shell command inside an already running container.
    ///
    /// # Panics
    pub fn shell_exec(&self, command: impl AsRef<str>) -> LogOutput {
        self.test_context.runner.tokio_runtime.block_on(async {
            let create_exec_result = self
                .test_context
                .runner
                .docker
                .create_exec(
                    &self.container_name,
                    CreateExecOptions {
                        cmd: Some(vec![CNB_LAUNCHER_PATH, SHELL_PATH, "-c", command.as_ref()]),
                        attach_stdout: Some(true),
                        ..CreateExecOptions::default()
                    },
                )
                .await
                .unwrap();

            let start_exec_result = self
                .test_context
                .runner
                .docker
                .start_exec(&create_exec_result.id, None)
                .await
                .unwrap();

            match start_exec_result {
                StartExecResults::Attached { output, .. } => {
                    log::consume_container_log_output(output)
                        .await
                        .expect("Could not consume container log output")
                }
                StartExecResults::Detached => LogOutput::default(),
            }
        })
    }
}

impl<'a> Drop for ContainerContext<'a> {
    fn drop(&mut self) {
        // We do not care if container removal succeeded or not. Panicking here would result in
        // SIGILL since this function might be called in a Tokio runtime.
        let _remove_container_result = self.test_context.runner.tokio_runtime.block_on(
            self.test_context.runner.docker.remove_container(
                &self.container_name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..RemoveContainerOptions::default()
                }),
            ),
        );
    }
}

const CNB_LAUNCHER_PATH: &str = "/cnb/lifecycle/launcher";
const SHELL_PATH: &str = "/bin/sh";
