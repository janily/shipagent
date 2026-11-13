//! Native sidebar rendering fixture. Backend results are explicitly supplied for display only.
use agentkit_store::entities::{AppState, SessionListStore, SessionStore, WorkspaceStore};
use agentkit_ui::{WorkbenchIntent, WorkbenchView, sidebar::SidebarIntent};
use gpui_kit::{
    component::{Theme, ThemeMode, ThemeRegistry, TitleBar},
    *,
};
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            agentkit_ui::set_locale("zh-CN");
            let seed: serde_json::Value =
                match serde_json::from_str(include_str!("fixtures/session-views.json")) {
                    Ok(s) => s,
                    Err(_) => {
                        cx.quit();
                        return;
                    }
                };
            let _ = ThemeRegistry::global_mut(cx)
                .load_themes_from_str(include_str!("../../agentkit-app/theme.json"));
            let themes = ThemeRegistry::global(cx).themes();
            if let (Some(light), Some(dark)) = (
                themes.get(seed["theme_light"].as_str().unwrap_or_default()),
                themes.get(seed["theme_dark"].as_str().unwrap_or_default()),
            ) {
                let (light, dark) = (light.clone(), dark.clone());
                Theme::update(cx, |t| {
                    t.light_theme = light;
                    t.dark_theme = dark;
                });
            }
            Theme::change(
                if std::env::var_os("AGENTKIT_VIEWS_DARK").is_some() {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                },
                None,
                cx,
            );
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
                ..TitleBar::window_options()
            };
            let _ = gpui_kit::open_window(options, cx, |window, cx| {
                let sessions: Vec<agentkit_protocol::generated::Session> =
                    serde_json::from_value(seed["sessions"].clone()).unwrap_or_default();
                let Some(session) = sessions.first().cloned() else {
                    return cx.new(|cx| WorkbenchView::new("Workbench".into(), window, cx));
                };
                let list = cx.new(|_| SessionListStore::default());
                list.update(cx, |l, cx| {
                    l.model.plans = sessions
                        .iter()
                        .filter(|s| {
                            s.last_message_role
                                == Some(agentkit_protocol::generated::SessionLastMessageRole::Plan)
                        })
                        .map(|s| s.id.clone())
                        .collect();
                    l.replace(sessions, cx);
                });
                let store = cx.new(|_| SessionStore::new(session));
                let app = cx.new(|_| AppState::default());
                let workspace = cx.new(|_| WorkspaceStore {
                    id: "fixture".into(),
                    name: seed["workspace"].as_str().unwrap_or_default().into(),
                    ..Default::default()
                });
                let view = cx.new(|cx| {
                    let mut v = WorkbenchView::new("Workbench".into(), window, cx);
                    v.connected(
                        seed["workspace"].as_str().unwrap_or_default().into(),
                        seed["model"].as_str().map(str::to_owned),
                        cx,
                    );
                    v.bind_list(list, cx);
                    v.bind_context(app, workspace, cx);
                    v.bind_session(store, window, cx);
                    v
                });
                let matches = seed["matches"].clone();
                cx.subscribe(&view, move |view, event: &WorkbenchIntent, cx| {
                    if let WorkbenchIntent::Sidebar(SidebarIntent::Search { request, query }) =
                        event
                    {
                        view.update(cx, |v, cx| {
                            v.search_results(
                                *request,
                                query,
                                Ok(if query.contains("reconnect") {
                                    serde_json::from_value(matches.clone()).unwrap_or_default()
                                } else {
                                    vec![]
                                }),
                                cx,
                            )
                        });
                    }
                })
                .detach();
                let focus = view.clone();
                window.defer(cx, move |window, cx| {
                    focus.update(cx, |v, cx| v.command_focus(true, true, window, cx))
                });
                view
            });
            cx.activate(true);
        });
}
