//! Native draft restoration fixture. No provider requests or backend writes.
use agentkit_store::{core::attachments::Entry, entities::SessionStore};
use agentkit_ui::WorkbenchView;
use base64::{Engine as _, engine::general_purpose::STANDARD};
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
            if std::env::var_os("AGENTKIT_DRAFT_DARK").is_some() {
                Theme::change(ThemeMode::Dark, None, cx);
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let seed: serde_json::Value =
                match serde_json::from_str(include_str!("fixtures/drafts.json")) {
                    Ok(s) => s,
                    Err(_) => {
                        cx.quit();
                        return;
                    }
                };
            let session = match serde_json::from_value(seed["session"].clone()) {
                Ok(s) => s,
                Err(_) => {
                    cx.quit();
                    return;
                }
            };
            let bytes = include_bytes!("fixtures/attachment-preview.png");
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
                ..TitleBar::window_options()
            };
            let _ = gpui_kit::open_window(options, cx, |window, cx| {
                let store = cx.new(|_| {
                    let mut s = SessionStore::new(session);
                    s.model
                        .set_draft(seed["draft"].as_str().unwrap_or_default().into());
                    for (index, value) in seed["files"].as_array().into_iter().flatten().enumerate()
                    {
                        if let Ok(mut file) = serde_json::from_value::<
                            agentkit_protocol::generated::FileAttachment,
                        >(value.clone())
                        {
                            if index == 0 {
                                file.thumbnail_base64 = Some(STANDARD.encode(bytes));
                            }
                            s.attachments.files.push(Entry {
                                id: index.to_string(),
                                file: std::sync::Arc::new(file),
                            });
                        }
                    }
                    s.draft_sync
                        .unavailable
                        .push(agentkit_store::core::drafts::Unavailable {
                            id: "missing".into(),
                            reference: std::sync::Arc::new(
                                agentkit_protocol::generated::SessionDraftAttachmentsItem {
                                    path: seed["missing"]["path"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .into(),
                                    name: seed["missing"]["name"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .into(),
                                    content: None,
                                },
                            ),
                        });
                    s
                });
                cx.new(|cx| {
                    let mut v = WorkbenchView::new("Workbench".into(), window, cx);
                    v.connected(
                        "Default".into(),
                        seed["model"].as_str().map(str::to_owned),
                        cx,
                    );
                    v.bind_session(store, window, cx);
                    v
                })
            });
            cx.activate(true);
        });
}
