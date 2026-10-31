//! Native package orchestration; signing secrets are read by the child tool only.
use anyhow::{Context, Result, ensure};
use std::{fs, path::Path, process::Command};

pub fn run(args: &[String]) -> Result<()> {
    let mut target = None;
    let mut sign = None;
    let mut probe = false;
    let mut notarize = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--target" | "--sign" => {
                let value = args
                    .get(i + 1)
                    .context("Missing package option value")?
                    .clone();
                if args[i] == "--target" {
                    ensure!(target.replace(value).is_none(), "Duplicate target");
                } else {
                    ensure!(sign.replace(value).is_none(), "Duplicate signing mode");
                }
                i += 2;
            }
            "--probe" => {
                ensure!(!probe, "Duplicate probe flag");
                probe = true;
                i += 1;
            }
            "--notarize" => {
                ensure!(!notarize, "Duplicate notarize flag");
                notarize = true;
                i += 1;
            }
            _ => anyhow::bail!("Unknown package option"),
        }
    }
    let target = target.context("Explicit --target macos-arm64|macos-x64 required")?;
    let sign = sign.context("Explicit --sign ad-hoc|developer-id required")?;
    ensure!(
        ["macos-arm64", "macos-x64"].contains(&target.as_str()),
        "Unsupported package target"
    );
    ensure!(
        ["ad-hoc", "developer-id"].contains(&sign.as_str()),
        "Unsupported signing mode"
    );
    ensure!(
        !notarize || sign == "developer-id",
        "Ad-hoc packages cannot be notarized"
    );
    ensure!(
        std::env::consts::OS == "macos",
        "macOS packages require native macOS host"
    );
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    ensure!(
        target == format!("macos-{arch}"),
        "Package target must match host"
    );
    if sign == "developer-id" {
        let identity =
            std::env::var("APPLE_SIGNING_IDENTITY").context("APPLE_SIGNING_IDENTITY required")?;
        ensure!(
            identity.starts_with("Developer ID Application:"),
            "Developer ID Application identity required"
        );
    }
    if notarize {
        ensure!(
            std::env::var_os("APPLE_NOTARY_KEYCHAIN_PROFILE").is_some(),
            "APPLE_NOTARY_KEYCHAIN_PROFILE required"
        );
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("Workspace root unavailable")?;
    ensure!(
        !repo.join("target/packages").join(&target).exists(),
        "Package output exists; preserve or move it before rebuilding"
    );
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(repo.join("Cargo.toml"))?)?;
    let version = manifest["workspace"]["package"]["version"]
        .as_str()
        .context("Workspace version unavailable")?;
    let brand: toml::Value =
        toml::from_str(&fs::read_to_string(repo.join("apps/workbench/brand.toml"))?)?;
    let data = serde_json::json!({"brand":brand,"version":version,"probe":probe,"backend":repo.join(format!("target/backend/darwin-{arch}")),"binary":repo.join("target/release/workbench"),"license":repo.join("LICENSE")});
    let config = tempfile::NamedTempFile::new()?;
    fs::write(config.path(), serde_json::to_vec(&data)?)?;
    // Validate all brand values before network/build side effects.
    ensure!(Command::new("node").args(["--input-type=module", "-e", "import {validateOptions} from './tools/package/bundle.mjs';import{readFileSync}from'node:fs';const c=JSON.parse(readFileSync(process.argv[1]));validateOptions({...c,target:process.argv[2],sign:process.argv[3],notarize:process.argv[4]==='true'});"]).arg(config.path()).arg(&target).arg(&sign).arg(notarize.to_string()).current_dir(repo).status()?.success(), "Invalid package options/branding");
    crate::backend::build()?;
    let mut build = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    build
        .args(["build", "--release", "--locked", "-p", "workbench"])
        .current_dir(repo)
        .env("MACOSX_DEPLOYMENT_TARGET", "13.0");
    if probe {
        build.args(["--features", "perf"]);
    }
    ensure!(build.status()?.success(), "Release build failed");
    ensure!(
        Command::new("node")
            .arg(repo.join("tools/package/macos.mjs"))
            .args(["--target", &target, "--sign", &sign, "--config"])
            .arg(config.path())
            .args([
                "--probe",
                &probe.to_string(),
                "--notarize",
                &notarize.to_string()
            ])
            .current_dir(repo)
            .status()?
            .success(),
        "macOS packaging failed"
    );
    Ok(())
}
