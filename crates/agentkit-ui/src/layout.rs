//! Serializable client-only shell layout policy.
use serde::{Deserialize, Serialize};
/// Right-side context tab. Full workflows are implemented by their controllers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelTab {
    /// Working directory and file context.
    #[default]
    Files,
    /// Submitted plan context.
    Plan,
    /// Observed background work.
    Tasks,
}
/// Persisted shell preferences, independent of viewport-driven collapse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellPreferences {
    /// Expanded sidebar width, clamped to 240–420px.
    pub sidebar_width: f32,
    /// User's sidebar visibility preference.
    pub sidebar_open: bool,
    /// Expanded right-panel width, clamped to 280–560px.
    pub right_width: f32,
    /// User's right-panel visibility preference.
    pub right_open: bool,
    /// Selected right-panel tab.
    pub panel: PanelTab,
    /// Hide both side regions temporarily.
    pub focus_mode: bool,
}
impl Default for ShellPreferences {
    fn default() -> Self {
        Self {
            sidebar_width: 280.,
            sidebar_open: true,
            right_width: 360.,
            right_open: false,
            panel: PanelTab::Files,
            focus_mode: false,
        }
    }
}
impl ShellPreferences {
    /// Clamp untrusted values loaded from disk without persisting auto-collapse.
    pub fn validate(&mut self) {
        self.sidebar_width = if self.sidebar_width.is_finite() {
            self.sidebar_width.clamp(240., 420.)
        } else {
            280.
        };
        self.right_width = if self.right_width.is_finite() {
            self.right_width.clamp(280., 560.)
        } else {
            360.
        };
    }
    /// Whether the side column is visible at this viewport width.
    pub fn sidebar_visible(&self, width: f32) -> bool {
        self.sidebar_open && !self.focus_mode && width >= 1100.
    }
    /// Whether the context region is visible, either inline or as a sheet.
    pub fn right_visible(&self) -> bool {
        self.right_open && !self.focus_mode
    }
    /// Use a modal sheet instead of consuming the chat column at narrow widths.
    pub fn right_is_sheet(&self, width: f32) -> bool {
        self.right_visible() && width < 1280.
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn breakpoints_do_not_mutate_user_preferences() {
        let mut p = ShellPreferences {
            right_open: true,
            ..Default::default()
        };
        assert!(!p.sidebar_visible(1099.));
        assert!(p.sidebar_visible(1100.));
        assert!(p.right_is_sheet(1279.));
        assert!(!p.right_is_sheet(1280.));
        p.focus_mode = true;
        assert!(!p.sidebar_visible(1440.));
        assert!(!p.right_visible());
        assert!(p.sidebar_open && p.right_open);
    }
    #[test]
    fn malformed_dimensions_are_clamped_to_supported_ranges() {
        let mut p = ShellPreferences {
            sidebar_width: f32::NAN,
            right_width: f32::INFINITY,
            ..Default::default()
        };
        p.validate();
        assert_eq!((p.sidebar_width, p.right_width), (280., 360.));
        p.sidebar_width = 900.;
        p.right_width = -5.;
        p.validate();
        assert_eq!((p.sidebar_width, p.right_width), (420., 280.));
    }
}
