use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    *,
};
/// Values submitted by a native navigation form.
#[derive(Clone)]
pub struct NavigationForm {
    /// Trimmed display name.
    pub name: String,
    /// Selected local workspace directory, empty for session rename.
    pub directory: String,
}
/// Native rename/workspace form, retained on failure so the user can retry.
pub struct NavigationDialog {
    name: Entity<InputState>,
    directory: Entity<InputState>,
    workspace: bool,
    busy: bool,
    error: Option<String>,
    _subscription: Subscription,
}
impl EventEmitter<NavigationForm> for NavigationDialog {}
impl NavigationDialog {
    /// Build a native form. Workspace creation exposes a system directory picker.
    pub fn new(name: &str, workspace: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let directory = cx.new(|cx| InputState::new(window, cx));
        let subscription = cx.subscribe_in(&name, window, |this, _, e, _, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                this.submit(cx);
            }
        });
        Self {
            name,
            directory,
            workspace,
            busy: false,
            error: None,
            _subscription: subscription,
        }
    }
    /// Focus the name input when the dialog opens.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.name.update(cx, |s, cx| s.focus(window, cx));
    }
    /// Gate duplicate submissions and show a recoverable error.
    pub fn status(&mut self, busy: bool, error: Option<String>, cx: &mut Context<Self>) {
        self.busy = busy;
        self.error = error;
        cx.notify();
    }
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.name.read(cx).value().trim().to_string();
        let directory = self.directory.read(cx).value().trim().to_string();
        if name.is_empty() || self.workspace && directory.is_empty() {
            self.error = Some(t!("ak.nav.required"));
            cx.notify();
            return;
        }
        self.busy = true;
        self.error = None;
        cx.emit(NavigationForm { name, directory });
        cx.notify();
    }
    fn pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("ak.nav.choose_directory").into()),
        });
        self.busy = true;
        cx.notify();
        cx.spawn_in(window, async move |weak, cx| {
            let result = paths.await;
            let _ = weak.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let directory = this.directory.clone();
                            let value = path.to_string_lossy().into_owned();
                            directory.update(cx, |s, cx| s.set_value(value, window, cx));
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.error = Some(t!("ak.nav.picker_failed")),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for NavigationDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(t!("ak.nav.name"))
            .child(
                Input::new(&self.name)
                    .id("navigation-name")
                    .aria_label(t!("ak.nav.name"))
                    .disabled(self.busy),
            )
            .when(self.workspace, |v| {
                v.child(t!("ak.nav.directory"))
                    .child(
                        Input::new(&self.directory)
                            .id("navigation-directory")
                            .aria_label(t!("ak.nav.directory"))
                            .disabled(true),
                    )
                    .child(
                        Button::new("navigation-pick")
                            .label(t!("ak.nav.choose_directory"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.pick(window, cx))),
                    )
            })
            .when_some(self.error.clone(), |v, error| {
                v.child(div().text_color(cx.theme().danger).child(error))
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("navigation-cancel")
                            .outline()
                            .label(t!("common.cancel"))
                            .disabled(self.busy)
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
                    .child(
                        Button::new("navigation-submit")
                            .primary()
                            .label(t!("ak.nav.save"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
    }
}
