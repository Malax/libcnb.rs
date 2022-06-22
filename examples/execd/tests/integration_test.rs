//! Integration tests using libcnb-test.
//!
//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

// Enable Clippy lints that are disabled by default.
// https://rust-lang.github.io/rust-clippy/stable/index.html
#![warn(clippy::pedantic)]

use libcnb_test::{assert_contains, TestConfig, TestRunner};

#[test]
#[ignore]
fn basic() {
    TestRunner::default().run_test(
        TestConfig::new("heroku/builder:22", "test-fixtures/empty-app"),
        |context| {
            context
                .prepare_container()
                .start_with_shell_command("env", |container| {
                    let env_stdout = container.logs_wait().stdout;

                    assert_contains!(env_stdout, "ROLL_1D6=");
                    assert_contains!(env_stdout, "ROLL_4D6=");
                    assert_contains!(env_stdout, "ROLL_1D20=");
                });
        },
    );
}
