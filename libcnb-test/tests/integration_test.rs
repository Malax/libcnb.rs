//! Integration tests using libcnb-test.
//!
//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

// Enable Clippy lints that are disabled by default.
// https://rust-lang.github.io/rust-clippy/stable/index.html
#![warn(clippy::pedantic)]

use libcnb_test::{BuildpackReference, TestConfig, TestRunner};
use tempfile::tempdir;

#[test]
#[ignore]
#[should_panic(expected = "pack command failed with exit-code 1!

pack stdout:


pack stderr:
ERROR: failed to build: failed to fetch builder image 'index.docker.io/libcnb/void-builder:doesntexist': image 'index.docker.io/libcnb/void-builder:doesntexist' does not exist on the daemon: not found
")]
fn panic_on_unsuccessful_pack_run() {
    let temp_app_dir = tempdir().unwrap();

    TestRunner::default().run_test(
        TestConfig::new("libcnb/void-builder:doesntexist", temp_app_dir.path())
            .buildpacks(vec![BuildpackReference::Other(String::from("libcnb/void"))]),
        |_| {},
    );
}
