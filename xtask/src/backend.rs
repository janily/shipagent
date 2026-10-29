//! Pinned upstream build in a disposable checkout; the submodule stays untouched.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
const PIN: &str = "6e8ebc456b6b82441be09118ca11afca79b84836";

fn checked(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    ensure!(
        command
            .status()
            .with_context(|| format!("Unable to start {program}"))?
            .success(),
        "{program} failed"
    );
    Ok(())
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn validate_artifacts(root: &Path) -> Result<()> {
    let runtime = if cfg!(windows) { "bun.exe" } else { "bun" };
    let rg = if cfg!(windows) { "rg.exe" } else { "rg" };
    for file in [
        PathBuf::from("server.js"),
        PathBuf::from("resources/pi-agent-server/index.js"),
        PathBuf::from("vendor/bun").join(runtime),
        PathBuf::from("node_modules/@vscode/ripgrep/bin").join(rg),
        PathBuf::from("node_modules/xlsx/package.json"),
    ] {
        let path = root.join(file);
        ensure!(
            path.is_file(),
            "Missing backend artifact: {}",
            path.display()
        );
    }
    ensure!(
        root.join("resources/scripts").is_dir(),
        "Missing document scripts"
    );
    ensure!(!root.join("webui").exists(), "WebUI must not be included");
    Ok(())
}

fn publish(staged: &Path, destination: &Path) -> Result<()> {
    let backup = destination.with_extension("previous");
    ensure!(
        !backup.exists(),
        "Previous build backup exists: {}; inspect before retrying",
        backup.display()
    );
    let had_previous = destination.exists();
    if had_previous {
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(staged, destination) {
        if had_previous {
            fs::rename(&backup, destination).context("Unable to restore previous backend")?;
        }
        return Err(error.into());
    }
    if had_previous {
        fs::remove_dir_all(backup)?;
    }
    Ok(())
}

pub fn build() -> Result<()> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("Missing workspace root")?;
    let upstream = repo.join("backend/mkagent");
    let version = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&upstream)
        .output()
        .context("Initialize backend/mkagent with git submodule update --init")?;
    ensure!(
        version.status.success() && String::from_utf8_lossy(&version.stdout).trim() == PIN,
        "Backend commit does not match pinned v0.2.6"
    );
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&upstream)
        .output()?;
    ensure!(
        dirty.status.success() && dirty.stdout.is_empty(),
        "Backend submodule has local changes; preserve them before building"
    );
    let bun = std::env::var_os("AGENTKIT_BUN").unwrap_or_else(|| "bun".into());
    let mut build_paths = Vec::new();
    if let Some(parent) = Path::new(&bun)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        build_paths.push(parent.to_owned());
    }
    build_paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let build_path = std::env::join_paths(build_paths)?;
    let output = Command::new(&bun)
        .arg("--version")
        .output()
        .context("Install Bun >=1.3.14 or set AGENTKIT_BUN")?;
    let bun_version = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<u32> = bun_version
        .trim()
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .context("Invalid Bun version")?;
    ensure!(
        output.status.success() && parts.as_slice() >= [1, 3, 14].as_slice(),
        "Bun >=1.3.14 required"
    );
    let target = repo.join("target");
    let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let scratch = Scratch(target.join(format!("backend-work-{}-{id}", std::process::id())));
    let source = scratch.0.join("source");
    fs::create_dir_all(&source)?;
    let archive = scratch.0.join("source.tar");
    checked(
        Command::new("git")
            .arg("archive")
            .arg("--format=tar")
            .arg(format!("--output={}", archive.display()))
            .arg(PIN)
            .current_dir(&upstream),
    )?;
    checked(
        Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&source),
    )?;
    let mut patches = fs::read_dir(repo.join("backend/patches"))?
        .map(|p| p.map(|p| p.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    checked(Command::new("git").args(["init", "--quiet"]).arg(&source))?;
    patches.retain(|p| p.extension().is_some_and(|e| e == "patch"));
    patches.sort();
    for patch in patches {
        checked(
            Command::new("git")
                .args(["apply", "--check"])
                .arg(&patch)
                .current_dir(&source),
        )?;
        checked(
            Command::new("git")
                .arg("apply")
                .arg(&patch)
                .current_dir(&source),
        )?;
    }
    // Optional public registry substitution only in this disposable source tree.
    // Versions and integrity hashes remain unchanged; upstream is never rewritten.
    if std::env::var("AGENTKIT_USE_NPMJS").as_deref() == Ok("1") {
        let lock = source.join("bun.lock");
        fs::write(
            &lock,
            fs::read_to_string(&lock)?.replace(
                "https://repo.huaweicloud.com/repository/npm/",
                "https://registry.npmjs.org/",
            ),
        )?;
    }
    let mut install = Command::new(&bun);
    install
        .args(["install", "--frozen-lockfile"])
        .current_dir(&source)
        .env("PATH", &build_path)
        .env("ELECTRON_SKIP_BINARY_DOWNLOAD", "1")
        .env("BUN_INSTALL_CACHE_DIR", target.join("bun-cache"))
        .env("npm_config_cache", target.join("npm-cache"));
    checked(&mut install)?;
    let built = scratch.0.join("built");
    checked(
        Command::new(&bun)
            .args(["run", "scripts/build-server.ts", "--no-webui"])
            .arg(format!("--output={}", built.display()))
            .env("PATH", &build_path)
            .current_dir(&source),
    )?;
    validate_artifacts(&built)?;
    // Retain upstream attribution alongside the distributable resources.
    for file in ["LICENSE", "NOTICE"] {
        fs::copy(source.join(file), built.join(file))?;
    }
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        "linux" => "linux",
        _ => bail!("Unsupported backend host"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => bail!("Unsupported backend architecture"),
    };
    let destination = target.join("backend").join(format!("{platform}-{arch}"));
    fs::create_dir_all(destination.parent().context("Missing backend parent")?)?;
    publish(&built, &destination)?;
    println!("Backend ready: {}", destination.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_publication_restores_previous_build() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let destination = dir.path().join("backend");
        fs::create_dir(&destination)?;
        fs::write(destination.join("server.js"), "previous")?;
        assert!(publish(&dir.path().join("missing"), &destination).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("server.js"))?,
            "previous"
        );
        Ok(())
    }
    #[test]
    fn missing_runtime_rejects_artifacts() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("server.js"), "bundle")?;
        assert!(validate_artifacts(dir.path()).is_err());
        Ok(())
    }
}
