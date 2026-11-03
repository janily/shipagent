//! A profile-scoped local focus endpoint, guarded by an OS-released file lock.
use interprocess::ConnectWaitMode;
#[cfg(unix)]
use interprocess::local_socket::ConnectOptions;
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{
    Listener, ListenerNonblockingMode, ListenerOptions, Stream, prelude::*,
};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};
/// Local instance negotiation failure; no backend has started at this point.
#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    /// A local filesystem or socket operation failed.
    #[error("Unable to establish the application instance")]
    Io(#[from] std::io::Error),
    /// Existing owner did not acknowledge focus within the bounded deadline.
    #[error("Existing application instance did not acknowledge focus")]
    Unresponsive {
        /// Last handshake stage attempted; no request contents are retained.
        phase: &'static str,
        /// Last IO failure, if the peer supplied none of the expected acknowledgement.
        #[source]
        cause: Option<std::io::Error>,
    },
}
/// Result of negotiating ownership before opening a window or starting a sidecar.
pub enum Claim {
    /// This process owns the profile and receives validated focus requests.
    Primary(Instance),
    /// Existing instance acknowledged focus; this process must exit.
    Forwarded,
}
/// Keeps the listener and ownership lock alive while GPUI handles focus requests.
pub struct Instance {
    /// Validated, coalesced focus notifications; no external URLs are executed.
    pub focus: async_channel::Receiver<()>,
    _worker: smol::Task<()>,
    _lock: std::sync::Arc<File>,
}
struct Server {
    listener: Listener,
    _lock: std::sync::Arc<File>,
}
fn socket_name<'a>(
    dir: &'a Path,
    _bundle: &str,
) -> Result<interprocess::local_socket::Name<'a>, InstanceError> {
    #[cfg(unix)]
    {
        Ok(dir
            .join("app.sock")
            .to_fs_name::<GenericFilePath>()?
            .into_owned())
    }
    #[cfg(windows)]
    {
        Ok(pipe_key(dir, _bundle)
            .to_ns_name::<GenericNamespaced>()?
            .into_owned())
    }
}
#[cfg(windows)]
fn pipe_key(dir: &Path, bundle: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{bundle}.{:x}",
        Sha256::digest(dir.to_string_lossy().as_bytes())
    )
}
/// Claim one frontend per client data directory, or forward focus to its owner.
/// Called before the GPUI event loop. A held lock prevents live socket replacement.
pub fn claim(dir: &Path, bundle: &str) -> Result<Claim, InstanceError> {
    std::fs::create_dir_all(dir)?;
    let dir = std::fs::canonicalize(dir)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("app.lock"))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut phase = "connect";
            let mut cause = None;
            while Instant::now() < deadline {
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(200));
                #[cfg(unix)]
                let connection = ConnectOptions::new()
                    .name(socket_name(&dir, bundle)?)
                    .wait_mode(ConnectWaitMode::Timeout(remaining))
                    .nonblocking_stream(true)
                    .connect_sync();
                // interprocess2.4.7's generic Windows socket constructor drops
                // wait_mode. The safe pipe constructor honours the connect deadline.
                #[cfg(windows)]
                let connection = interprocess::os::windows::named_pipe::DuplexPipeStream::<
                    interprocess::os::windows::named_pipe::pipe_mode::Bytes,
                >::connect_by_path_with_wait_mode(
                    format!(r"\\.\pipe\{}", pipe_key(&dir, bundle)),
                    ConnectWaitMode::Timeout(remaining),
                );
                match connection {
                    Ok(mut stream) => {
                        stream.set_nonblocking(true)?;
                        let until = (Instant::now() + Duration::from_millis(200)).min(deadline);
                        phase = "write";
                        match write_deadline(&mut stream, b"{\"cmd\":\"focus\"}\n", until) {
                            Ok(()) => {
                                phase = "read";
                                match read_line(&mut stream, until) {
                                    Ok(bytes) if bytes == b"{\"ok\":true}\n" => {
                                        return Ok(Claim::Forwarded);
                                    }
                                    Ok(_) => cause = Some(std::io::ErrorKind::InvalidData.into()),
                                    Err(e) => cause = Some(e),
                                }
                            }
                            Err(e) => cause = Some(e),
                        }
                    }
                    Err(e) => {
                        phase = "connect";
                        cause = Some(e);
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            return Err(InstanceError::Unresponsive { phase, cause });
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let socket = dir.join("app.sock");
        match std::fs::symlink_metadata(&socket) {
            Ok(meta) if meta.file_type().is_socket() => std::fs::remove_file(socket)?,
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "Application socket path is not a socket",
                )
                .into());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    let listener = ListenerOptions::new()
        .name(socket_name(&dir, bundle)?)
        .nonblocking(ListenerNonblockingMode::Accept)
        .create_sync()?;
    let lock = std::sync::Arc::new(file);
    let server = Server {
        listener,
        _lock: lock.clone(),
    };
    let (tx, rx) = async_channel::bounded(1);
    let worker = smol::spawn(async move {
        loop {
            match server.listener.accept() {
                Ok(stream) => {
                    let valid = smol::unblock(move || read_focus(stream)).await;
                    if let Ok(mut stream) = valid {
                        let _ = tx.force_send(());
                        let _ = smol::unblock(move || {
                            write_deadline(
                                &mut stream,
                                b"{\"ok\":true}\n",
                                Instant::now() + Duration::from_millis(100),
                            )
                        })
                        .await;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    smol::Timer::after(Duration::from_millis(10)).await;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        drop(server);
    });
    Ok(Claim::Primary(Instance {
        focus: rx,
        _worker: worker,
        _lock: lock,
    }))
}
fn write_deadline(
    stream: &mut impl Write,
    mut bytes: &[u8],
    deadline: Instant,
) -> std::io::Result<()> {
    while !bytes.is_empty() && Instant::now() < deadline {
        match stream.write(bytes) {
            Ok(0) if cfg!(windows) => std::thread::sleep(Duration::from_millis(2)),
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(e) => return Err(e),
        }
    }
    if bytes.is_empty() {
        Ok(())
    } else {
        Err(std::io::ErrorKind::TimedOut.into())
    }
}
fn read_line(stream: &mut impl Read, deadline: Instant) -> std::io::Result<Vec<u8>> {
    read_line_policy(stream, deadline, cfg!(windows))
}
fn read_line_policy(
    stream: &mut impl Read,
    deadline: Instant,
    retry_empty: bool,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while Instant::now() < deadline && bytes.len() < 64 {
        let mut byte = [0];
        match stream.read(&mut byte) {
            // interprocess Windows byte pipes map ERROR_NO_DATA/BrokenPipe to
            // an empty read. Unlike Unix EOF, this can mean no bytes available yet.
            Ok(0) if retry_empty => std::thread::sleep(Duration::from_millis(2)),
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(_) => {
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    return Ok(bytes);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(e) => return Err(e),
        }
    }
    Err(std::io::ErrorKind::TimedOut.into())
}
fn read_focus(mut stream: Stream) -> std::io::Result<Stream> {
    stream.set_nonblocking(true)?;
    let bytes = read_line(&mut stream, Instant::now() + Duration::from_millis(250))?;
    if serde_json::from_slice::<serde_json::Value>(&bytes).ok()
        != Some(serde_json::json!({"cmd":"focus"}))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid focus request",
        ));
    }
    Ok(stream)
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn second_claim_forwards_without_stealing_owner() {
        let t = tempfile::tempdir().unwrap();
        let first = match claim(t.path(), "org.test.focus").unwrap() {
            Claim::Primary(i) => i,
            _ => panic!("not owner"),
        };
        assert!(matches!(
            claim(t.path(), "org.test.focus").unwrap(),
            Claim::Forwarded
        ));
        assert!(first.focus.try_recv().is_ok());
        drop(first);
    }
    #[test]
    fn independent_profiles_have_independent_owners() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        assert!(matches!(
            claim(a.path(), "org.test.focus").unwrap(),
            Claim::Primary(_)
        ));
        assert!(matches!(
            claim(b.path(), "org.test.focus").unwrap(),
            Claim::Primary(_)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn stale_socket_is_recovered_but_regular_files_are_preserved() {
        use std::os::unix::net::UnixListener;
        let t = tempfile::tempdir().unwrap();
        drop(UnixListener::bind(t.path().join("app.sock")).unwrap());
        let first = claim(t.path(), "org.test.stale").unwrap();
        assert!(matches!(first, Claim::Primary(_)));
        drop(first);
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("app.sock"), "KEEP").unwrap();
        assert!(claim(other.path(), "org.test.stale").is_err());
        assert_eq!(
            std::fs::read_to_string(other.path().join("app.sock")).unwrap(),
            "KEEP"
        );
    }
    #[test]
    #[ignore = "Owned cross-process focus probe, invoked by the parent test"]
    fn owned_instance_fixture() {
        let dir = std::path::PathBuf::from(std::env::var_os("AGENTKIT_INSTANCE_TEST_DIR").unwrap());
        match std::env::var("AGENTKIT_INSTANCE_TEST_MODE")
            .unwrap()
            .as_str()
        {
            "primary" => {
                let instance = match claim(&dir, "org.test.process").unwrap() {
                    Claim::Primary(i) => i,
                    _ => panic!("not primary"),
                };
                std::fs::write(dir.join("ready"), "ready").unwrap();
                smol::block_on(async {
                    smol::future::race(
                        async {
                            instance.focus.recv().await.unwrap();
                        },
                        async {
                            smol::Timer::after(Duration::from_secs(5)).await;
                            panic!("no focus request");
                        },
                    )
                    .await;
                });
                std::fs::write(dir.join("focused"), "focused").unwrap();
                std::thread::sleep(Duration::from_millis(100));
            }
            "secondary" => assert!(matches!(
                claim(&dir, "org.test.process").unwrap(),
                Claim::Forwarded
            )),
            _ => panic!("mode"),
        }
    }
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    #[test]
    fn a_separate_process_forwards_focus_to_the_existing_owner() {
        let t = tempfile::tempdir().unwrap();
        let spawn = |mode| {
            Child(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "instance::tests::owned_instance_fixture",
                    ])
                    .env("AGENTKIT_INSTANCE_TEST_DIR", t.path())
                    .env("AGENTKIT_INSTANCE_TEST_MODE", mode)
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            )
        };
        let mut primary = spawn("primary");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !t.path().join("ready").exists() {
            assert!(Instant::now() < deadline, "primary not ready");
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut secondary = spawn("secondary");
        loop {
            if let Some(status) = secondary.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "secondary did not return");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(t.path().join("focused").exists());
        loop {
            if let Some(status) = primary.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "primary did not stop");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    struct EmptyThenBytes {
        empty: bool,
        bytes: std::io::Cursor<Vec<u8>>,
    }
    impl Read for EmptyThenBytes {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.empty {
                self.empty = false;
                Ok(0)
            } else {
                self.bytes.read(buf)
            }
        }
    }
    #[test]
    fn windows_empty_pipe_reads_wait_for_ack_but_unix_eof_still_fails() {
        let bytes = b"{\"ok\":true}\n".to_vec();
        let mut pipe = EmptyThenBytes {
            empty: true,
            bytes: std::io::Cursor::new(bytes.clone()),
        };
        assert_eq!(
            read_line_policy(&mut pipe, Instant::now() + Duration::from_millis(100), true).unwrap(),
            bytes
        );
        let mut socket = EmptyThenBytes {
            empty: true,
            bytes: std::io::Cursor::new(bytes),
        };
        assert_eq!(
            read_line_policy(
                &mut socket,
                Instant::now() + Duration::from_millis(100),
                false
            )
            .unwrap_err()
            .kind(),
            std::io::ErrorKind::UnexpectedEof
        );
        assert_eq!(
            read_line_policy(
                &mut std::io::empty(),
                Instant::now() + Duration::from_millis(10),
                true
            )
            .unwrap_err()
            .kind(),
            std::io::ErrorKind::TimedOut
        );
    }
}
