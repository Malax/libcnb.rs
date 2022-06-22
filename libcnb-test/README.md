# libcnb-test

An experimental integration testing framework for Cloud Native Buildpacks written in Rust with libcnb.rs.

## Experimental

This crate is marked as experimental. It currently implements the most basic building blocks for writing
integration tests with libcnb.rs. Its feature set is deliberately cut down to get ball rolling and get a better feel
which features are required. See [issues tagged with `libcnb-test`][libcnb-test-label] for possible future improvements.
Please use the same tag for feature requests.

[libcnb-test-label]: https://github.com/heroku/libcnb.rs/labels/libcnb-test

## Example

```rust,no_run
// In $CRATE_ROOT/tests/integration_test.rs
use libcnb_test::{assert_contains, BuildpackReference, TestRunner, TestConfig};

#[test]
fn test() {
    TestRunner::default().run_test(
        TestConfig::new("heroku/builder:22", "test-fixtures/app"),
        |context| {
            assert_contains!(context.pack_stdout, "---> Maven Buildpack");
            assert_contains!(context.pack_stdout, "---> Installing Maven");
            assert_contains!(context.pack_stdout, "---> Running mvn package");

            context
                .prepare_container()
                .expose_port(12345)
                .start_with_default_process(|container| {
                    assert_eq!(
                        call_test_fixture_service(
                            container.address_for_port(12345).unwrap(),
                            "Hagbard Celine"
                        )
                        .unwrap(),
                        "enileC drabgaH"
                    );
                });
        },
    );
}

fn call_test_fixture_service(addr: std::net::SocketAddr, payload: &str) -> Result<String, ()> {
    unimplemented!()
}
```

## Known issues

- Only local Docker daemons are fully supported. If using Circle CI you must use the
  [`machine` executor](https://circleci.com/docs/2.0/executor-types/#using-machine) rather
  than the [remote docker](https://circleci.com/docs/2.0/building-docker-images/) feature.
