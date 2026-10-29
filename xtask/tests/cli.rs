#![allow(clippy::unwrap_used)]
#[test]
fn unknown_command_fails_instead_of_reporting_success() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("unsupported")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Unsupported command"));
}
#[test]
fn help_describes_backend_build() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("backend build"));
}
