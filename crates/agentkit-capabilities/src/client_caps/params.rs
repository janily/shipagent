use super::failure;
use agentkit_protocol::{RpcValue, WireError};
use serde::Deserialize;
use std::path::PathBuf;
/// Validated native operation; wire arguments never reach OS commands directly.
#[derive(Clone, Debug)]
pub enum Action {
    /// HTTP(S) or mailto URL.
    OpenExternal(String),
    /// Local absolute path.
    OpenPath(PathBuf),
    /// Local absolute path to reveal.
    ShowItemInFolder(PathBuf),
    /// Modal choice preserving backend button indices.
    Confirm(ConfirmSpec),
    /// Native file/folder selection.
    OpenFile(FileSpec),
}
/// Backend message box options from mkagent v0.2.6 transport/capabilities.ts.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmSpec {
    /// Semantic severity, optional.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Backend supplied title.
    pub title: String,
    /// Primary message.
    pub message: String,
    /// Optional explanation.
    pub detail: Option<String>,
    /// Labels in original response order.
    pub buttons: Vec<String>,
    /// Initially focused button, defaults to zero.
    pub default_id: Option<usize>,
    /// Dismissal response, defaults to last button.
    pub cancel_id: Option<usize>,
}
impl ConfirmSpec {
    /// Initial focus target.
    pub fn default_index(&self) -> usize {
        self.default_id.unwrap_or(0)
    }
    /// Index returned for Escape or closing.
    pub fn cancel_index(&self) -> usize {
        self.cancel_id
            .unwrap_or(self.buttons.len().saturating_sub(1))
    }
}
/// File picker extension group.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileFilter {
    /// Display label.
    pub name: String,
    /// Extensions without leading dots; * means all files.
    pub extensions: Vec<String>,
}
/// Backend file picker options.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileSpec {
    /// Native picker title.
    pub title: Option<String>,
    /// Initial directory or filename.
    pub default_path: Option<PathBuf>,
    /// openFile, openDirectory, multiSelections.
    #[serde(default)]
    pub properties: Vec<String>,
    /// Named extension groups.
    #[serde(default)]
    pub filters: Vec<FileFilter>,
}
impl FileSpec {
    /// Permit directory selection.
    pub fn directories(&self) -> bool {
        self.properties.iter().any(|p| p == "openDirectory")
    }
    /// Permit file selection (default unless only directories requested).
    pub fn files(&self) -> bool {
        !self.directories() || self.properties.iter().any(|p| p == "openFile")
    }
    /// Permit more than one selection.
    pub fn multiple(&self) -> bool {
        self.properties.iter().any(|p| p == "multiSelections")
    }
}
/// Validate the complete argument shape before queueing a foreground operation.
pub fn parse(channel: &str, args: Vec<RpcValue>) -> Result<Action, WireError> {
    if args.len() != 1 {
        return Err(failure("Expected one capability argument"));
    }
    let value =
        serde_json::to_value(&args[0]).map_err(|_| failure("Invalid capability argument"))?;
    let path = || -> Result<PathBuf, WireError> {
        let text = value
            .as_str()
            .ok_or_else(|| failure("Expected a local path"))?;
        let path = PathBuf::from(text);
        if !path.is_absolute() || text.contains('\0') {
            return Err(failure("Expected a local absolute path"));
        }
        Ok(path)
    };
    match channel {
        "client:openExternal" => {
            let text = value.as_str().ok_or_else(|| failure("Expected URL"))?;
            let url = url::Url::parse(text).map_err(|_| failure("Invalid URL"))?;
            if !matches!(url.scheme(), "http" | "https" | "mailto")
                || (url.scheme() != "mailto" && url.host_str().is_none())
                || text.contains(['\0', '\r', '\n'])
            {
                return Err(failure("URL scheme not allowed"));
            }
            Ok(Action::OpenExternal(url.into()))
        }
        "client:openPath" => Ok(Action::OpenPath(path()?)),
        "client:showItemInFolder" => Ok(Action::ShowItemInFolder(path()?)),
        "client:confirmDialog" => {
            let spec: ConfirmSpec = serde_json::from_value(value)
                .map_err(|_| failure("Invalid confirmation options"))?;
            if spec.buttons.is_empty()
                || spec.buttons.iter().any(|s| s.is_empty())
                || spec.default_index() >= spec.buttons.len()
                || spec.cancel_index() >= spec.buttons.len()
                || spec.kind.as_ref().is_some_and(|k| {
                    !matches!(
                        k.as_str(),
                        "none" | "info" | "warning" | "error" | "question"
                    )
                })
            {
                return Err(failure("Invalid confirmation buttons or type"));
            }
            Ok(Action::Confirm(spec))
        }
        "client:openFileDialog" => {
            let spec: FileSpec = serde_json::from_value(value)
                .map_err(|_| failure("Invalid file picker options"))?;
            if spec.properties.iter().any(|p| {
                !matches!(
                    p.as_str(),
                    "openFile" | "openDirectory" | "multiSelections" | "createDirectory"
                )
            }) || spec.default_path.as_ref().is_some_and(|p| !p.is_absolute())
                || spec
                    .filters
                    .iter()
                    .flat_map(|f| &f.extensions)
                    .any(|s| s.is_empty() || s.contains(['/', '\\', '\0']))
            {
                return Err(failure("Unsupported file picker options"));
            }
            Ok(Action::OpenFile(spec))
        }
        _ => Err(super::unavailable("Client capability unavailable")),
    }
}
