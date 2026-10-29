#![allow(clippy::unwrap_used)]
#[test]
fn codegen_help_describes_reproducible_drift_check() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("codegen --check"));
}
