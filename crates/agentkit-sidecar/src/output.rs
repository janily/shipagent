//! Bounded line decoding, token filtering, stderr tail and rotating diagnostics.
use crate::{SidecarError, lock, parse_ready_line};
use secrecy::{ExposeSecret, SecretString};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc::Sender},
};
const LIMIT: usize = 64 * 1024;
const LOG_LIMIT: u64 = 2 * 1024 * 1024;
struct Log {
    file: File,
    path: PathBuf,
    size: u64,
}
pub(crate) struct Output {
    log: Mutex<Log>,
    pub tail: Mutex<String>,
    pub occupied: Mutex<Option<Option<u32>>>,
    secrets: Vec<SecretString>,
}
fn open(path: &std::path::Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
impl Output {
    pub fn new(
        directory: &std::path::Path,
        secrets: Vec<SecretString>,
    ) -> Result<Arc<Self>, SidecarError> {
        std::fs::create_dir_all(directory)?;
        let path = directory.join("sidecar.log");
        let file = open(&path)?;
        let size = file.metadata()?.len();
        Ok(Arc::new(Self {
            log: Mutex::new(Log { file, path, size }),
            tail: Mutex::new(String::new()),
            occupied: Mutex::new(None),
            secrets,
        }))
    }
    fn sanitize(&self, line: &str) -> Option<String> {
        if line.contains("MKAGENT_SERVER_TOKEN=") {
            return None;
        }
        let mut text = line.to_owned();
        for secret in &self.secrets {
            if !secret.expose_secret().is_empty() {
                text = text.replace(secret.expose_secret(), "[REDACTED]");
            }
        }
        Some(text)
    }
    fn line(&self, line: &str, stderr: bool) {
        let Some(text) = self.sanitize(line) else {
            return;
        };
        if stderr {
            if let Some(rest) = text
                .split("Another server instance is already running")
                .nth(1)
            {
                let pid = rest
                    .split("PID ")
                    .nth(1)
                    .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
                    .and_then(|s| s.parse().ok());
                *lock(&self.occupied) = Some(pid);
            }
            let mut tail = lock(&self.tail);
            tail.push_str(&text);
            tail.push('\n');
            if tail.len() > LIMIT {
                let mut cut = tail.len() - LIMIT;
                while !tail.is_char_boundary(cut) {
                    cut += 1;
                }
                tail.drain(..cut);
            }
        }
        let mut log = lock(&self.log);
        // Retain one previous 2MiB file; failed logging never blocks pipe drainage.
        let result = (|| -> std::io::Result<()> {
            if log.size >= LOG_LIMIT {
                // Close the old file before rename on Windows.
                let fresh = log.path.with_extension("next");
                let mut options = OpenOptions::new();
                options.create(true).truncate(true).write(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let replacement = options.open(&fresh)?;
                let old = std::mem::replace(&mut log.file, replacement);
                drop(old);
                let previous = log.path.with_extension("log.1");
                let _ = std::fs::remove_file(&previous);
                std::fs::rename(&log.path, previous)?;
                std::fs::rename(fresh, &log.path)?;
                log.size = 0;
            }
            writeln!(
                log.file,
                "{}: {}",
                if stderr { "stderr" } else { "stdout" },
                text
            )?;
            log.size += text.len() as u64 + 10;
            Ok(())
        })();
        if result.is_err() {
            tracing::warn!("Sidecar diagnostic write failed; continuing bounded pipe drainage");
        }
    }
    pub fn drain(
        self: Arc<Self>,
        mut reader: impl Read,
        stderr: bool,
        ready: Sender<Result<String, SidecarError>>,
    ) {
        let mut chunk = [0u8; 4096];
        let mut line = Vec::new();
        let mut sent = false;
        let mut truncated = false;
        loop {
            let count = match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(count) => count,
            };
            for byte in &chunk[..count] {
                if *byte == b'\n' {
                    self.consume(&line, stderr, &ready, &mut sent, truncated);
                    line.clear();
                    truncated = false;
                } else if line.len() < LIMIT {
                    line.push(*byte);
                } else {
                    truncated = true;
                }
            }
        }
        if !line.is_empty() {
            self.consume(&line, stderr, &ready, &mut sent, truncated);
        }
    }
    fn consume(
        &self,
        bytes: &[u8],
        stderr: bool,
        ready: &Sender<Result<String, SidecarError>>,
        sent: &mut bool,
        truncated: bool,
    ) {
        let text = String::from_utf8_lossy(bytes);
        let text = text.trim_end_matches('\r');
        if !stderr && !*sent {
            match parse_ready_line(text) {
                Ok(Some(url)) => {
                    let _ = ready.send(Ok(url));
                    *sent = true;
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                    *sent = true;
                }
                Ok(None) => {}
            }
        }
        if truncated {
            self.line("[oversized sidecar line omitted]", stderr);
        } else {
            self.line(text, stderr);
        }
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn logs_and_tails_bound_lines_and_remove_tokens_on_both_pipes() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::new(dir.path(), vec!["private-token".into()]).unwrap();
        let (send, _) = std::sync::mpsc::channel();
        output.clone().drain(
            std::io::Cursor::new(format!(
                "MKAGENT_SERVER_TOKEN=private-token\nnoise private-token\n{}\n",
                "密".repeat(100000)
            )),
            true,
            send,
        );
        let text = std::fs::read_to_string(dir.path().join("sidecar.log")).unwrap();
        assert!(!text.contains("private-token"));
        assert!(!text.contains("MKAGENT_SERVER_TOKEN="));
        assert!(text.contains("[REDACTED]"));
        assert!(lock(&output.tail).len() <= LIMIT);
    }
    #[test]
    fn log_rotation_retains_one_private_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::new(dir.path(), vec![]).unwrap();
        output.line("previous generation", false);
        lock(&output.log).size = LOG_LIMIT;
        output.line("current generation", false);
        assert!(
            std::fs::read_to_string(dir.path().join("sidecar.log.1"))
                .unwrap()
                .contains("previous generation")
        );
        assert!(
            std::fs::read_to_string(dir.path().join("sidecar.log"))
                .unwrap()
                .contains("current generation")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.path().join("sidecar.log"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn oversized_lines_cannot_leak_a_token_fragment_at_the_truncation_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::new(dir.path(), vec!["CANARYFRAGMENT_PRIVATE".into()]).unwrap();
        let (send, _) = std::sync::mpsc::channel();
        let input = format!("{}CANARYFRAGMENT_PRIVATE\n", "x".repeat(LIMIT - 8));
        output.drain(std::io::Cursor::new(input), true, send);
        assert!(
            !std::fs::read_to_string(dir.path().join("sidecar.log"))
                .unwrap()
                .contains("CANARYFR")
        );
    }
}
