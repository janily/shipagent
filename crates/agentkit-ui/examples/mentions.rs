//! Native render fixture for mention menus; no backend or provider requests.
use agentkit_store::entities::{AppState, SessionStore, WorkspaceStore};
use agentkit_ui::{WorkbenchIntent, WorkbenchView};
use gpui_kit::{
    component::{Theme, ThemeMode, TitleBar},
    *,
};
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            agentkit_ui::set_locale("zh-CN");
            let dark = std::env::var_os("AGENTKIT_MENTIONS_DARK").is_some();
            if dark {
                Theme::change(ThemeMode::Dark, None, cx);
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let seed: serde_json::Value =
                match serde_json::from_str(include_str!("fixtures/mentions.json")) {
                    Ok(s) => s,
                    Err(_) => {
                        cx.quit();
                        return;
                    }
                };
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
                ..TitleBar::window_options()
            };
            let _ = gpui_kit::open_window(options, cx, |window, cx| {
                let session = match serde_json::from_value(seed["session"].clone()) {
                    Ok(s) => s,
                    Err(_) => unreachable!(),
                };
                let store = cx.new(|_| SessionStore::new(session));
                let app = cx.new(|_| AppState {
                    connections: serde_json::from_value(serde_json::json!([seed["connection"]]))
                        .unwrap_or_default(),
                    ..Default::default()
                });
                let workspace = cx.new(|_| WorkspaceStore {
                    workspace: serde_json::from_value(seed["workspace"].clone()).ok(),
                    skills: serde_json::from_value(seed["skills"].clone()).unwrap_or_default(),
                    ..Default::default()
                });
                let view = cx.new(|cx| {
                    let mut v = WorkbenchView::new("Workbench".into(), window, cx);
                    v.connected(
                        seed["workspace"]["name"]
                            .as_str()
                            .unwrap_or_default()
                            .into(),
                        seed["model"].as_str().map(str::to_owned),
                        cx,
                    );
                    v.bind_context(app, workspace, cx);
                    v.bind_session(store, window, cx);
                    v
                });
                let files = seed["files"].clone();
                let handle = window.window_handle();
                cx.subscribe(&view, move |view, event: &WorkbenchIntent, cx| {
                    if let WorkbenchIntent::SearchMentions {
                        session_id,
                        request,
                        query,
                    } = event
                    {
                        let _ = handle.update(cx, |_, window, cx| {
                            view.update(cx, |v, cx| {
                                v.mention_results(
                                    session_id,
                                    *request,
                                    query,
                                    serde_json::from_value(files.clone())
                                        .map_err(|e| e.to_string()),
                                    window,
                                    cx,
                                )
                            })
                        });
                    }
                })
                .detach();
                let focus = view.clone();
                window.defer(cx, move |window, cx| {
                    let input = focus.read(cx).composer_input(cx);
                    input.update(cx, |input, cx| {
                        input.focus_handle(cx).focus(window, cx);
                        let text = if dark { "/rev" } else { "@src" };
                        input.replace_text_in_range(None, text, window, cx);
                    });
                });
                view
            });
            cx.activate(true);
        });
}
