# Changelog

## [Unreleased]

- Leverage `Into` trait for `String`/`&str` arguments in `ContainerContext` ([#412](https://github.com/heroku/libcnb.rs/pull/412))
- Pass `--trust-builder` to `pack build` to ensure the builders used in tests are always trusted ([#409](https://github.com/heroku/libcnb.rs/pull/409))
- Add `IntegrationTest::app_dir_preprocessor`, allowing users to modify the app directory before an integration test run ([#397](https://github.com/heroku/libcnb.rs/pull/397))
- Update `bollard` dependency from 0.12.0 to 0.13.0 ([#419](https://github.com/heroku/libcnb.rs/pull/419))
- Rename `IntegrationTest` to `TestConfig`. ([#422](https://github.com/heroku/libcnb.rs/pull/422))
- Rename `IntegrationTestContext` to `TestContext`. ([#422](https://github.com/heroku/libcnb.rs/pull/422))

## [0.3.1] 2022-04-12

- Update project URLs for the GitHub repository move to the `heroku` org ([#388](https://github.com/heroku/libcnb.rs/pull/388)).

## [0.3.0] 2022-03-08

- Add `IntegrationTest::env` and `IntegrationTest::envs`, allowing users to set environment variables for the build process. ([#346](https://github.com/heroku/libcnb.rs/pull/346))
- Replaced `IntegrationTestContext::start_container` with `IntegrationTestContext::prepare_container`, allowing users to configure the container before starting it. Ports can now be exposed via `PrepareContainerContext::expose_port`. ([#346](https://github.com/heroku/libcnb.rs/pull/346))
- Added the ability to set environment variables for the container via `PrepareContainerContext::env` and `PrepareContainerContext::envs`. ([#346](https://github.com/heroku/libcnb.rs/pull/346))
- Switch from `libcnb-cargo` to the new `libcnb-package` crate for buildpack packaging, which improves compile times due to it not including CLI-related dependencies ([#362](https://github.com/heroku/libcnb.rs/pull/362)).
- Use `--pull-policy if-not-present` when running `pack build` ([#373](https://github.com/heroku/libcnb.rs/pull/373)).
- Allow starting a container without using the default process type. Removed `PrepareContainerContext::start`, added `PrepareContainerContext::start_with_default_process`, `PrepareContainerContext::start_with_default_process_args`, `PrepareContainerContext::start_with_process`, `PrepareContainerContext::start_with_process_args`, `PrepareContainerContext::start_with_shell_command` ([#366](https://github.com/heroku/libcnb.rs/pull/366))
- Add `ContainerContext::logs_now` and `ContainerContext::logs_wait` to access the logs of the container. Useful when used in conjunction with the new `PrepareContainerContext::start_with_shell_command` method to get the shell command output. ([#366](https://github.com/heroku/libcnb.rs/pull/366))
- Replaced `container_context::ContainerExecResult` with `log::LogOutput` which is now also returned by `ContainerContext::logs` and `ContainerContext::logs_follow`. ([#366](https://github.com/heroku/libcnb.rs/pull/366))
- Move support for connecting to the Docker daemon over TLS behind a new `remote-docker` feature flag, since remote Docker support is not fully implemented and pulls in many additional dependencies ([#376](https://github.com/heroku/libcnb.rs/pull/376)).

## [0.2.0] 2022-02-28

- `libcnb-test` now cross-compiles and packages all binary targets of the buildpack for an integration test. The main buildpack binary is either the only binary target or the target with the same name as the crate. This feature allows the usage of additional binaries for i.e. execd. ([#314](https://github.com/heroku/libcnb.rs/pull/314))
- Increase minimum supported Rust version from 1.56 to 1.58 ([#318](https://github.com/heroku/libcnb.rs/pull/318)).
- Add `assert_contains!` macro for easier matching of `pack` output in integration tests. ([#322](https://github.com/heroku/libcnb.rs/pull/322))
- Fail tests early with a clearer error message, if expected cross-compilation toolchains are not found ([#347](https://github.com/heroku/libcnb.rs/pull/347)).
- Update `libcnb-cargo` from `0.2.1` to `0.3.0` - see the [libcnb-cargo changelog](../libcnb-cargo/CHANGELOG.md#030-2022-02-28). ([#361](https://github.com/heroku/libcnb.rs/pull/361))

## [0.1.1] 2022-02-04

- Use the `DOCKER_HOST` environment variable to determine the Docker connection strategy, adding support for HTTPS 
connections in addition to local UNIX sockets. This enables using `libcnb-test` in more complex setups like on CircleCI 
where the Docker daemon is on a remote machine. ([#305](https://github.com/heroku/libcnb.rs/pull/305))

## [0.1.0] 2022-02-02

- Initial release ([#277](https://github.com/heroku/libcnb.rs/pull/277)).
