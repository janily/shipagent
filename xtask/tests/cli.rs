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
fn invalid_package_requests_fail_before_building_or_signing() {
    for args in [
        vec!["package"],
        vec!["package", "--target", "linux-x64", "--sign", "ad-hoc"],
        vec![
            "package",
            "--target",
            "macos-arm64",
            "--sign",
            "ad-hoc",
            "--notarize",
        ],
        vec![
            "package",
            "--target",
            "macos-arm64",
            "--target",
            "macos-x64",
            "--sign",
            "ad-hoc",
        ],
    ] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
    }
}
