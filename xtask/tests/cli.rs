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

#[test]
fn package_rejects_implicit_or_invalid_signing_before_building() {
    for args in [
        vec!["package"],
        vec!["package", "--target", "../escape", "--sign", "ad-hoc"],
        vec!["package", "--target", "macos-arm64", "--sign", "unsigned"],
        vec![
            "package",
            "--target",
            "macos-arm64",
            "--sign",
            "ad-hoc",
            "--notarize",
        ],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Backend ready"));
    }
}
