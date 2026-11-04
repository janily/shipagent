use super::*;
impl WorkbenchController {
    pub(crate) fn apply_display_preferences(&self, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.preferences.value.borrow().clone();
        agentkit_ui::set_locale(&p.language);
        match p.appearance {
            Appearance::System => {
                gpui_kit::component::Theme::sync_system_appearance(Some(window), cx)
            }
            Appearance::Light => gpui_kit::component::Theme::change(
                gpui_kit::component::ThemeMode::Light,
                Some(window),
                cx,
            ),
            Appearance::Dark => gpui_kit::component::Theme::change(
                gpui_kit::component::ThemeMode::Dark,
                Some(window),
                cx,
            ),
        }
        gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(p.font_size as f32));
        let prefs = DisplayPreferences {
            appearance: p.appearance,
            language: p.language.clone(),
            font_size: p.font_size,
            stream_fade: p.stream_fade,
            restore: p.restore_session,
            send_key: p.send_key,
        };
        self.app_state.update(cx, |a, cx| {
            a.language = p.language;
            a.theme = format!("{:?}", p.appearance).to_lowercase();
            cx.notify();
        });
        self.view.update(cx, |v, cx| {
            v.set_send_key(p.send_key, cx);
            v.set_display(p.font_size, p.stream_fade, cx);
            v.localize(window, cx);
            v.settings().update(cx, |s, cx| s.preferences(prefs, cx));
        });
        cx.refresh_windows();
    }
    pub(super) fn local_setting(
        &mut self,
        change: LocalChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.update(|p| match &change {
            LocalChange::Appearance(v) => p.appearance = *v,
            LocalChange::Language(v) => p.language = v.clone(),
            LocalChange::Font(v) => p.font_size = *v,
            LocalChange::Fade(v) => p.stream_fade = *v,
            LocalChange::Restore(v) => p.restore_session = *v,
            LocalChange::SendKey(v) => p.send_key = *v,
        });
        self.apply_display_preferences(window, cx);
        if let LocalChange::SendKey(key) = change
            && let Some(api) = self.settings_api()
        {
            let epoch = self.epoch;
            self.operations.push(cx.spawn(async move |weak, cx| {
                let key = match key {
                    agentkit_ui::composer::SendKey::Enter => {
                        agentkit_client::api::types::SendMessageKey::Enter
                    }
                    agentkit_ui::composer::SendKey::CmdEnter => {
                        agentkit_client::api::types::SendMessageKey::CmdEnter
                    }
                };
                if let Err(e) = api.send_key(key).await {
                    let _ = weak.update(cx, |this, cx| {
                        if this.epoch == epoch {
                            this.settings_status(false, Some(e.to_string()), None, cx);
                        }
                    });
                }
            }));
        }
    }
}
