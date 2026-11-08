//! Native settings visual verification with explicitly local fixture data.
use agentkit_protocol::generated::PermissionMode;
use agentkit_store::entities::AppState;
use agentkit_ui::settings::{BackendPreferences, ProxyValues, SettingsView};
use gpui_kit::{
    component::{Theme, ThemeMode, TitleBar},
    *,
};

fn main() {
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| {
        gpui_kit::init(cx);
        agentkit_ui::set_locale(&std::env::var("AGENTKIT_LANGUAGE").unwrap_or_else(|_| "zh-CN".into()));
        if std::env::var("AGENTKIT_SETTINGS_SAMPLE_DARK").as_deref()==Ok("1") { Theme::change(ThemeMode::Dark,None,cx); }
        let page = std::env::var("AGENTKIT_SETTINGS_SAMPLE_PAGE").ok().and_then(|s|s.parse().ok()).unwrap_or(4);
        cx.on_window_closed(|cx,_| { if cx.windows().is_empty() { cx.quit(); } }).detach();
        let narrow = std::env::var_os("AGENTKIT_SETTINGS_SAMPLE_NARROW").is_some();
        let dimensions = if narrow { size(px(960.),px(600.)) } else { size(px(1280.),px(820.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::centered(dimensions,cx)), window_min_size: Some(size(px(960.),px(600.))), ..TitleBar::window_options() };
        let result = gpui_kit::open_window(options,cx,|window,cx| {
            window.set_window_title(&agentkit_ui::t!("ak.settings"));
            let app = cx.new(|_| AppState { connected:true,..Default::default() });
            cx.new(|cx| {
                let mut view = SettingsView::new(cx);
                view.bind(app,cx);
                view.backend_preferences(BackendPreferences { proxy: ProxyValues { enabled:true,http_proxy:"http://127.0.0.1:8080".into(),..Default::default() }, keep_awake:true, workspace_id:"visual-fixture".into(), permission_mode:PermissionMode::Safe,permission_policy:"{\n  \"version\": \"1\",\n  \"allowedBashPatterns\": [\"git status\"],\n  \"blockedTools\": []\n}".into(),restart_available:true },window,cx);
                view.preview_page(page,cx);
                view
            })
        });
        if result.is_err() { cx.quit(); }
        cx.activate(true);
    });
}
