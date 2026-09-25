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

#[test]
fn validate_json_reports_flag_count() {
    let output = flagset()
        .args(["tests/fixtures/valid.conf", "validate", "--json"])
        .output()
        .expect("failed to run flagset binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), r#"{"ok":true,"flags":3}"#);
}

#[test]
fn validate_json_reports_parse_errors_as_json() {
    let output = flagset()
        .args(["tests/fixtures/broken.conf", "validate", "--json"])
        .output()
        .expect("failed to run flagset binary");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""ok":false"#) && stdout.contains("rollout must be 0..=100"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn check_json_reports_flag_key_and_result() {
    let output = flagset()
        .args([
            "tests/fixtures/valid.conf",
            "check",
            "dark_mode",
            "user-482",
            "--json",
        ])
        .output()
        .expect("failed to run flagset binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        r#"{"flag":"dark_mode","key":"user-482","enabled":true}"#
    );
}

#[test]
fn list_json_produces_one_entry_per_flag() {
    let output = flagset()
        .args(["tests/fixtures/valid.conf", "list", "--json"])
        .output()
        .expect("failed to run flagset binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(trimmed.starts_with('[') && trimmed.ends_with(']'));
    assert!(trimmed.contains(r#""name":"checkout_v2""#));
    assert!(trimmed.contains(r#""rollout":25"#));
    assert!(trimmed.contains(r#""expires":null"#));
    assert_eq!(trimmed.matches("\"name\"").count(), 3);
}
