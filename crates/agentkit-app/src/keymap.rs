//! Validated user keymap; fixed scopes protect input, sidebar and approval contexts.
use crate::actions::*;
use gpui_kit::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};
mod catalog;
pub use catalog::label;
use catalog::{Entry, defaults};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    bindings: Vec<Override>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Override {
    action: String,
    key: Option<String>,
}
/// Installed keymap and its safe, displayable validation state.
pub struct State {
    /// User-owned configuration file.
    pub path: PathBuf,
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    /// True when the original file was invalid or unreadable; defaults remain installed.
    pub invalid: bool,
    bindings: Vec<(String, String)>,
    rows: Vec<(String, String, String)>,
}
impl Global for State {}
/// Read a bounded UTF-8 file without overwriting invalid user input.
pub fn read(path: &Path) -> Result<Option<String>, ()> {
    use std::io::Read;
    match std::fs::File::open(path) {
        Ok(f) => {
            let mut s = String::new();
            f.take(262145).read_to_string(&mut s).map_err(|_| ())?;
            if s.len() > 262144 {
                return Err(());
            }
            Ok(Some(s))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}
fn binding(e: &Entry, cx: &App) -> Result<KeyBinding, ()> {
    let context = if e.context.is_empty() {
        None
    } else {
        Some(std::rc::Rc::new(
            KeyBindingContextPredicate::parse(e.context).map_err(|_| ())?,
        ))
    };
    KeyBinding::load(&e.key, e.action.boxed_clone(), context, false, None, cx).map_err(|_| ())
}
fn resolve(source: Option<&str>, cx: &App) -> Result<Vec<Entry>, ()> {
    let mut entries = defaults();
    if let Some(source) = source {
        let document: Document = serde_json::from_str(source).map_err(|_| ())?;
        if document.version != 1 || document.bindings.len() > 64 {
            return Err(());
        }
        let mut seen = std::collections::HashSet::new();
        for item in document.bindings {
            if !seen.insert(item.action.clone()) {
                return Err(());
            }
            let Some(index) = entries.iter().position(|e| e.id == item.action) else {
                return Err(());
            };
            if let Some(key) = item.key {
                if key.trim().is_empty() || key.len() > 128 {
                    return Err(());
                }
                entries[index].key = key;
            } else {
                entries.remove(index);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for e in &entries {
        let b = binding(e, cx)?;
        if b.keystrokes().is_empty() || !seen.insert((e.context, format!("{:?}", b.keystrokes()))) {
            return Err(());
        }
    }
    Ok(entries)
}
/// Install defaults or a fully validated override document. Reinstallation removes old custom keys.
pub fn install(path: PathBuf, source: Result<Option<String>, ()>, cx: &mut App) {
    let revision = cx
        .try_global::<State>()
        .map_or(1, |s| s.revision.wrapping_add(1));
    let generation = cx.try_global::<State>().map_or(0, |s| s.generation);
    let resolved = source.and_then(|s| resolve(s.as_deref(), cx));
    let invalid = resolved.is_err();
    let entries = resolved.unwrap_or_else(|_| defaults());
    let old = cx
        .try_global::<State>()
        .map(|s| s.bindings.clone())
        .unwrap_or_default();
    let mut removed = old;
    removed.extend(
        defaults()
            .iter()
            .map(|e| (e.key.clone(), e.context.to_string())),
    );
    removed.push(("backspace".into(), "SessionSidebar".into()));
    for (key, scope) in removed {
        let predicate = if scope.is_empty() {
            None
        } else {
            KeyBindingContextPredicate::parse(&scope)
                .ok()
                .map(std::rc::Rc::new)
        };
        if let Ok(marker) = KeyBinding::load(&key, Box::new(NoAction), predicate, false, None, cx) {
            cx.bind_keys([marker]);
        }
    }
    let mut bindings: Vec<_> = entries
        .iter()
        .map(|e| (e.key.clone(), e.context.to_string()))
        .collect();
    let resolved: Vec<_> = entries.iter().filter_map(|e| binding(e, cx).ok()).collect();
    let rows = entries
        .iter()
        .map(|e| (e.id.into(), e.key.clone(), e.context.into()))
        .collect();
    cx.bind_keys(resolved);
    if entries
        .iter()
        .any(|e| e.id == "sidebar::Delete" && e.key == "delete")
    {
        cx.bind_keys([KeyBinding::new(
            "backspace",
            agentkit_ui::sidebar::DeleteAction,
            Some("SessionSidebar"),
        )]);
        bindings.push(("backspace".into(), "SessionSidebar".into()));
    }
    cx.set_global(State {
        revision,
        generation,
        path,
        invalid,
        bindings,
        rows,
    });
    crate::menus::keymap_changed(cx);
}
/// Effective translated rows for the read-only settings page.
pub fn rows(cx: &App) -> Vec<(String, String, String)> {
    cx.try_global::<State>()
        .map(|s| {
            s.rows
                .iter()
                .map(|(id, key, scope)| {
                    (
                        label(id),
                        key.clone(),
                        match scope.as_str() {
                            "" => agentkit_ui::t!("ak.menu.scope_global"),
                            "PermissionPrompt" => agentkit_ui::t!("ak.menu.scope_approval"),
                            "SessionSidebar" => agentkit_ui::t!("ak.menu.scope_sidebar"),
                            "Composer > Input" => agentkit_ui::t!("ak.menu.scope_input"),
                            _ => agentkit_ui::t!("ak.menu.scope_window"),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}
#[cfg(test)]
#[path = "keymap/tests.rs"]
mod tests;

pub(crate) fn escape_stops(cx: &App) -> bool {
    cx.try_global::<State>().is_none_or(|s| {
        s.rows
            .iter()
            .any(|(id, key, _)| id == "chat::StopProcessing" && key == "escape")
    })
}

pub(crate) fn ensure_file(path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => return Ok(()),
        Ok(_) => return Err(std::io::Error::other("Keymap path is not a file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Keymap parent unavailable"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(b"{\n  \"version\": 1,\n  \"bindings\": []\n}\n")?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e.error),
    }
}
