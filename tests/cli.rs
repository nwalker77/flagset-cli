// Exercises the actual `flagset` binary, not the library, so a CI pipeline
// can trust that `validate`'s exit code means what the README says it means.
use std::process::Command;

fn flagset() -> Command {
    Command::new(env!("CARGO_BIN_EXE_flagset"))
}

#[test]
fn validate_succeeds_on_a_well_formed_config() {
    let output = flagset()
        .args(["tests/fixtures/valid.conf", "validate"])
        .output()
        .expect("failed to run flagset binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok: 3 flag(s)");
}

#[test]
fn validate_fails_on_a_malformed_config() {
    let output = flagset()
        .args(["tests/fixtures/broken.conf", "validate"])
        .output()
        .expect("failed to run flagset binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rollout must be 0..=100"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn validate_fails_when_the_config_file_is_missing() {
    let output = flagset()
        .args(["tests/fixtures/does_not_exist.conf", "validate"])
        .output()
        .expect("failed to run flagset binary");

    assert!(!output.status.success());
}
