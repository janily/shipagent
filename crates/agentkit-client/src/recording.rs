//! Explicit development-only session event recording. No RPC arguments are recorded.
use async_channel::Receiver;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use std::{fs::OpenOptions, io::Write, path::PathBuf};
/// Commands accepted by a single recording worker across connection generations.
pub enum RecordingInput {
    /// Subscribed session-event arguments, without RPC request envelopes.
    Events(Vec<Value>),
    /// Register a new generation token before recording its events.
    Redact(SecretString),
}
/// Background recording failure. Contents and paths are omitted from error display.
#[derive(Debug, thiserror::Error)]
pub enum RecordingError {
    /// Destination creation or writing failed.
    #[error("Event recording IO failed")]
    Io(#[from] std::io::Error),
    /// Event serialization failed.
    #[error("Event recording serialization failed")]
    Encode(#[from] serde_json::Error),
}
/// Record subscribed session-event arguments as JSONL using background file IO.
/// The destination must not exist. Callers must observe errors; dropping the future
/// unsubscribes. Content is opt-in debug data; supplied secret strings are redacted.
pub async fn record(
    receiver: Receiver<RecordingInput>,
    path: PathBuf,
) -> Result<(), RecordingError> {
    let mut file =
        smol::unblock(move || OpenOptions::new().write(true).create_new(true).open(path)).await?;
    let mut secrets = Vec::new();
    while let Ok(input) = receiver.recv().await {
        let args = match input {
            RecordingInput::Events(args) => args,
            RecordingInput::Redact(secret) => {
                secrets.push(secret);
                continue;
            }
        };
        let mut bytes = Vec::new();
        for value in args {
            let mut line = serde_json::to_string(&value)?;
            for secret in secrets
                .iter()
                .map(|s| s.expose_secret())
                .filter(|s| !s.is_empty())
            {
                // Match JSON-escaped secrets too (quotes/backslashes can occur in tokens).
                let escaped = serde_json::to_string(secret)?;
                line = line.replace(&escaped[1..escaped.len() - 1], "<REDACTED>");
            }
            bytes.extend_from_slice(line.as_bytes());
            bytes.push(b'\n');
        }
        file = smol::unblock(move || {
            file.write_all(&bytes)?;
            file.flush()?;
            Ok::<_, std::io::Error>(file)
        })
        .await?;
    }
    Ok(())
}
