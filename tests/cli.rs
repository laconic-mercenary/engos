//! CLI integration tests.
//!
//! These tests invoke the compiled `engos` binary as a subprocess using
//! `assert_cmd`. This catches argument parsing bugs and exit-code contracts
//! that unit tests of library functions cannot reach — the binary entrypoint
//! itself is under test here, not just the library logic.

// `assert_cmd::Command` is a thin wrapper around `std::process::Command` that
// adds Cargo-aware binary resolution and fluent assertion methods.
use assert_cmd::Command;

/// `engos --version` exits successfully and prints the version string.
///
/// The version must match the `version` field in `Cargo.toml`. This test will
/// fail if someone bumps the binary version without updating the assertion,
/// which is intentional — the version string is a contract with the operator.
#[test]
fn prints_version() {
    Command::cargo_bin("engos")
        .unwrap()
        // Pass the --version flag exactly as an operator would type it.
        .arg("--version")
        .assert()
        // Exit code 0 — `--version` is not an error condition.
        .success()
        // The output must contain the version string. `predicates::str::contains`
        // does a substring match so the test does not break if the binary adds
        // a trailing newline or ANSI codes in future.
        .stdout(predicates::str::contains("engos 0.1.0"));
}

/// Passing a path that does not exist exits with a non-zero code and a message.
///
/// A missing watch path is always a configuration error — there is no useful
/// behaviour to attempt, so the binary must fail loudly. This ensures the
/// operator gets a clear error rather than a silent hang.
#[test]
fn exits_with_error_on_missing_watch_path() {
    Command::cargo_bin("engos")
        .unwrap()
        // Use a path that cannot plausibly exist on any test machine.
        .arg("/tmp/this-path-does-not-exist-engos-test")
        .assert()
        // Any non-zero exit code is acceptable — we just need it to fail.
        .failure()
        // The error message must explain what went wrong. An operator debugging
        // a misconfigured setup should see "does not exist", not a panic trace.
        .stderr(predicates::str::contains("does not exist"));
}
