//! Login-shell environment is loaded once on macOS, never into the parent's global environment.
//! Adapted from mkagent v0.2.6 apps/electron/src/main/shell-env.ts; bounded NUL-separated capture.
use std::{ffi::OsString, process::Command};
pub(crate) fn apply(
    command: &mut Command,
    extra: &[(String, String)],
    root: &std::path::Path,
    platform: &str,
    arch: &str,
    stop: &std::sync::atomic::AtomicBool,
) -> std::io::Result<()> {
    let shell = snapshot(stop);
    command
        .envs(shell.iter().cloned())
        .envs(extra.iter().cloned());
    let base = extra
        .iter()
        .rev()
        .find(|(key, _)| key == "PATH")
        .map(|(_, value)| OsString::from(value))
        .or_else(|| {
            shell
                .iter()
                .find(|(key, _)| key == "PATH")
                .map(|(_, value)| value.clone())
        })
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    let mut paths = vec![
        root.join("resources/bin"),
        root.join(format!("resources/bin/{platform}-{arch}")),
        root.join("vendor/bun"),
    ];
    paths.extend(std::env::split_paths(&base));
    command.env(
        "PATH",
        std::env::join_paths(paths).map_err(std::io::Error::other)?,
    );
    // Remove every TLS override, including values imported from a login shell or extra_env.
    for (name, _) in std::env::vars_os()
        .chain(shell)
        .chain(extra.iter().map(|(k, v)| (k.into(), v.into())))
    {
        let key = name.to_string_lossy();
        if key.starts_with("MKAGENT_RPC_TLS_")
            || matches!(key.as_ref(), "MKAGENT_WEBUI_DIR" | "MKAGENT_HEALTH_PORT")
        {
            command.env_remove(name);
        }
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn snapshot(_stop: &std::sync::atomic::AtomicBool) -> Vec<(OsString, OsString)> {
    vec![]
}
#[cfg(target_os = "macos")]
fn snapshot(stop: &std::sync::atomic::AtomicBool) -> Vec<(OsString, OsString)> {
    static ENV: std::sync::OnceLock<Vec<(OsString, OsString)>> = std::sync::OnceLock::new();
    ENV.get_or_init(|| {
        shell_env(stop).unwrap_or_else(|| {
            tracing::warn!("Login shell environment unavailable; retaining inherited environment");
            vec![]
        })
    })
    .clone()
}
#[cfg(target_os = "macos")]
fn shell_env(stop: &std::sync::atomic::AtomicBool) -> Option<Vec<(OsString, OsString)>> {
    capture_shell(
        std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into()),
        stop,
    )
}
#[cfg(target_os = "macos")]
fn capture_shell(
    shell: OsString,
    stop: &std::sync::atomic::AtomicBool,
) -> Option<Vec<(OsString, OsString)>> {
    use std::{
        io::Read,
        os::unix::ffi::OsStringExt,
        process::Stdio,
        time::{Duration, Instant},
    };
    let mut command = Command::new(shell);
    command
        .args(["-ilc", "printf '__SHIPAGENT_ENV__\\0'; /usr/bin/env -0"])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for key in ["HOME", "USER", "SHELL", "TMPDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("TERM", "xterm-256color")
        .env("APPLE_SUPPRESS_DEVELOPER_TOOL_POPUP", "1")
        .env("GIT_TERMINAL_PROMPT", "0");
    let mut tree = crate::process::Tree::spawn(command).ok()?;
    let stdout = tree.inner().stdout.take()?;
    let (send, recv) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = vec![];
        let result = stdout.take(1024 * 1024 + 1).read_to_end(&mut bytes);
        let _ = send.send(result.map(|_| bytes));
    });
    let until = Instant::now() + Duration::from_secs(5);
    while tree.inner().try_wait().ok()?.is_none() {
        if stop.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Kill inherited-pipe descendants before waiting for the capture reader.
    tree.kill();
    let bytes = recv
        .recv_timeout(until.saturating_duration_since(Instant::now()))
        .ok()?
        .ok()?;
    if bytes.len() > 1024 * 1024 {
        return None;
    }
    let marker = b"__SHIPAGENT_ENV__\0";
    let start = bytes.windows(marker.len()).rposition(|s| s == marker)? + marker.len();
    Some(
        bytes[start..]
            .split(|b| *b == 0)
            .filter_map(|entry| {
                let eq = entry.iter().position(|b| *b == b'=')?;
                if eq == 0 || entry.starts_with(b"VITE_") {
                    return None;
                }
                Some((
                    OsString::from_vec(entry[..eq].to_vec()),
                    OsString::from_vec(entry[eq + 1..].to_vec()),
                ))
            })
            .collect(),
    )
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn real_zsh_accepts_the_login_query_and_nul_environment_capture() {
    let stop = std::sync::atomic::AtomicBool::new(false);
    let result = capture_shell("/bin/zsh".into(), &stop);
    assert!(result.is_some_and(|values| {
        values
            .iter()
            .any(|(key, value)| key == "PATH" && !value.is_empty())
    }));
}
