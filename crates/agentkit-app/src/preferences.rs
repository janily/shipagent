//! Client-owned preferences; no credentials or transcript content belongs here.
use agentkit_ui::layout::ShellPreferences;
pub use agentkit_ui::settings::Appearance;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
/// Restorable window geometry in logical desktop pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    /// Desktop x position.
    pub x: f32,
    /// Desktop y position.
    pub y: f32,
    /// Window width.
    pub width: f32,
    /// Window height.
    pub height: f32,
    /// Preserve maximized restore bounds.
    pub maximized: bool,
}
impl Geometry {
    /// Keep enough of the title bar on a connected display to recover the window.
    pub fn title_visible_on(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        self.x + self.width > x + 64.
            && self.x < x + width - 64.
            && self.y >= y
            && self.y + 40. <= y + height
    }
}
/// Version-one local preference document. Optional future fields remain ignorable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Backend browser default was successfully initialized for this profile.
    pub browser_initialized: bool,
    /// Backend acknowledged first-run setup deferral.
    pub onboarding_deferred: bool,
    /// File schema version.
    pub version: u32,
    /// Client appearance, never a backend theme channel.
    pub appearance: Appearance,
    /// Body font size in logical pixels.
    pub font_size: u8,
    /// Animate newly streamed text.
    pub stream_fade: bool,
    /// Owned locale: zh-CN or en.
    pub language: String,
    /// Client-owned Enter or Cmd/Ctrl+Enter send preference.
    pub send_key: agentkit_ui::composer::SendKey,
    /// Alert for approval requests arriving while the window is in the background.
    pub approval_notifications: bool,
    /// Alert when a background turn completes. Delivery is provided by the notification capability.
    pub completion_notifications: bool,
    /// Last window dimensions and position.
    pub geometry: Option<Geometry>,
    /// Fixed-column shell preferences.
    pub shell: ShellPreferences,
    /// Restore the last opened session at launch.
    pub restore_session: bool,
    /// Last workspace identity.
    pub workspace: Option<String>,
    /// Last session identity within that workspace.
    pub session: Option<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            browser_initialized: false,
            onboarding_deferred: false,
            version: 1,
            appearance: Appearance::Light,
            font_size: 14,
            stream_fade: true,
            language: "zh-CN".into(),
            send_key: Default::default(),
            approval_notifications: true,
            completion_notifications: true,
            geometry: None,
            shell: ShellPreferences::default(),
            restore_session: true,
            workspace: None,
            session: None,
        }
    }
}
impl Preferences {
    /// Validate persisted sizes and supported language without trusting arbitrary file values.
    pub fn validate(&mut self) {
        self.version = 1;
        self.shell.validate();
        self.font_size = self.font_size.clamp(12, 16);
        if self.language != "en" && self.language != "zh-CN" {
            self.language = "zh-CN".into();
        }
        if self.geometry.as_ref().is_some_and(|g| {
            ![g.x, g.y, g.width, g.height].iter().all(|v| v.is_finite())
                || g.width < 960.
                || g.height < 600.
                || g.width > 16384.
                || g.height > 16384.
        }) {
            self.geometry = None;
        }
    }
}
/// Preference file IO or decoding failure.
#[derive(Debug, thiserror::Error)]
pub enum PreferencesError {
    /// Disk operation failed.
    #[error("Preference storage failed")]
    Io(#[from] std::io::Error),
    /// Invalid preference document.
    #[error("Invalid preference document")]
    Json(#[from] serde_json::Error),
    /// A newer client owns this schema; do not overwrite it.
    #[error("Unsupported preference version")]
    Version,
}
/// Load before entering the GPUI event loop. Missing files use supplied brand defaults.
pub fn load(path: &Path, defaults: Preferences) -> Result<Preferences, PreferencesError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut value: Preferences = serde_json::from_slice(&bytes)?;
            if value.version != 1 {
                return Err(PreferencesError::Version);
            }
            value.validate();
            Ok(value)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(defaults),
        Err(e) => Err(e.into()),
    }
}
fn store(path: &Path, prefs: &Preferences) -> Result<(), PreferencesError> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Preference parent unavailable"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec_pretty(prefs)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
/// One ordered, coalescing background writer shared by shell and lifecycle observers.
type WriterSlot = Rc<RefCell<Option<smol::Task<Result<(), PreferencesError>>>>>;
#[derive(Clone)]
pub(crate) struct PreferenceStore {
    pub value: Rc<RefCell<Preferences>>,
    tx: Option<async_channel::Sender<Preferences>>,
    failures: async_channel::Receiver<()>,
    worker: WriterSlot,
}
impl PreferenceStore {
    pub fn readonly(initial: Preferences) -> Self {
        let (_, failures) = async_channel::bounded(1);
        Self {
            value: Rc::new(RefCell::new(initial)),
            failures,
            tx: None,
            worker: Rc::new(RefCell::new(None)),
        }
    }
    pub fn writable(&self) -> bool {
        self.tx.as_ref().is_some_and(|tx| !tx.is_closed())
    }
    pub fn new(path: PathBuf, initial: Preferences) -> Self {
        let (tx, rx) = async_channel::bounded::<Preferences>(1);
        let (error_tx, failures) = async_channel::bounded(1);
        let worker = smol::spawn(async move {
            let result = async {
                while let Ok(mut prefs) = rx.recv().await {
                    if !rx.is_closed() {
                        smol::Timer::after(Duration::from_millis(60)).await;
                    }
                    while let Ok(new) = rx.try_recv() {
                        prefs = new;
                    }
                    let next_path = path.clone();
                    smol::unblock(move || store(&next_path, &prefs)).await?;
                }
                Ok(())
            }
            .await;
            if result.is_err() {
                let _ = error_tx.try_send(());
            }
            result
        });
        Self {
            value: Rc::new(RefCell::new(initial)),
            tx: Some(tx),
            failures,
            worker: Rc::new(RefCell::new(Some(worker))),
        }
    }
    pub fn update(&self, change: impl FnOnce(&mut Preferences)) {
        let mut value = self.value.borrow_mut();
        change(&mut value);
        value.validate();
        if let Some(tx) = &self.tx {
            let _ = tx.force_send(value.clone());
        }
    }
    pub fn failures(&self) -> async_channel::Receiver<()> {
        self.failures.clone()
    }
    pub async fn finish(&self) -> Result<(), PreferencesError> {
        if let Some(tx) = &self.tx {
            tx.close();
        }
        let worker = self.worker.borrow_mut().take();
        if let Some(worker) = worker {
            worker.await?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn defaults_validation_and_future_schema_do_not_clobber() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("prefs.json");
        assert_eq!(
            load(&path, Preferences::default()).unwrap(),
            Preferences::default()
        );
        std::fs::write(&path, "{\"version\":99}").unwrap();
        assert!(matches!(
            load(&path, Preferences::default()),
            Err(PreferencesError::Version)
        ));
        let mut p = Preferences::default();
        p.shell.sidebar_width = 999.;
        p.language = "bad".into();
        p.geometry = Some(Geometry {
            x: 0.,
            y: 0.,
            width: 10.,
            height: 1.,
            maximized: false,
        });
        p.validate();
        assert_eq!(p.shell.sidebar_width, 420.);
        assert_eq!(p.language, "zh-CN");
        assert!(p.geometry.is_none());
    }
    #[test]
    fn atomic_latest_save_round_trips() {
        smol::block_on(async {
            let t = tempfile::tempdir().unwrap();
            let path = t.path().join("prefs.json");
            let store = PreferenceStore::new(path.clone(), Preferences::default());
            for i in 0..100 {
                store.update(|p| {
                    p.session = Some(format!("s{i}"));
                    p.shell.right_open = true;
                    p.send_key = agentkit_ui::composer::SendKey::CmdEnter;
                });
            }
            store.finish().await.unwrap();
            let p = load(&path, Preferences::default()).unwrap();
            assert_eq!(p.session.as_deref(), Some("s99"));
            assert!(p.shell.right_open);
            assert_eq!(p.send_key, agentkit_ui::composer::SendKey::CmdEnter);
            assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 1);
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod failure_tests {
    use super::*;
    #[test]
    fn failing_storage_reports_once_and_becomes_readonly() {
        smol::block_on(async {
            let t = tempfile::tempdir().unwrap();
            let blocked = t.path().join("file");
            std::fs::write(&blocked, "KEEP").unwrap();
            let store = PreferenceStore::new(blocked.join("prefs.json"), Preferences::default());
            let failures = store.failures();
            store.update(|p| p.shell.right_open = true);
            assert!(store.finish().await.is_err());
            assert!(failures.try_recv().is_ok());
            assert!(!store.writable());
            assert_eq!(std::fs::read_to_string(blocked).unwrap(), "KEEP");
        });
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    #[test]
    fn disconnected_or_above_screen_geometry_is_recentred() {
        let mut g = Geometry {
            x: 0.,
            y: 0.,
            width: 1280.,
            height: 820.,
            maximized: false,
        };
        assert!(g.title_visible_on(0., 0., 1440., 900.));
        g.y = -700.;
        assert!(!g.title_visible_on(0., 0., 1440., 900.));
        g.y = 0.;
        g.x = 3000.;
        assert!(!g.title_visible_on(0., 0., 1440., 900.));
        g.x = -300.;
        assert!(g.title_visible_on(0., 0., 1440., 900.));
    }
}

#[cfg(test)]
mod onboarding_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    #[test]
    fn legacy_preferences_start_uninitialized_and_acknowledgements_round_trip() {
        let old: Preferences = serde_json::from_str(r#"{"version":1,"language":"en"}"#).unwrap();
        assert!(!old.browser_initialized && !old.onboarding_deferred);
        assert!(old.completion_notifications && old.approval_notifications);
        let updated = Preferences {
            browser_initialized: true,
            onboarding_deferred: true,
            ..old
        };
        let restored: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&updated).unwrap()).unwrap();
        assert!(restored.browser_initialized && restored.onboarding_deferred);
    }
}
