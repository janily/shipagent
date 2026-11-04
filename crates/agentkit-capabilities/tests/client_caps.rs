#![allow(clippy::unwrap_used)]
use agentkit_capabilities::client_caps::NativeCapabilities;
use agentkit_protocol::{RpcValue, WireError};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, IntoElement, Point, Render, TestAppContext,
    Window, WindowBounds, WindowOptions, div, px, size, test::TestWindowExt,
};
use serde_json::{Value, json};
struct Empty;
impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
fn setup(cx: &mut TestAppContext) -> (NativeCapabilities, AnyWindowHandle) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        let caps = NativeCapabilities::install(cx);
        let (w, _) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1000.), px(700.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| Empty),
        )
        .unwrap();
        caps.remember(w);
        (caps, w)
    })
}
fn call(
    cx: &TestAppContext,
    caps: &NativeCapabilities,
    name: &str,
    value: Value,
) -> gpui_kit::Task<Result<RpcValue, WireError>> {
    let handler = caps.handlers()[name].clone();
    cx.executor()
        .spawn(async move { handler(vec![serde_json::from_value(value).unwrap()]).await })
}
fn pump(cx: &mut TestAppContext, mut ready: impl FnMut(&mut TestAppContext) -> bool) {
    for _ in 0..500 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
        cx.run_until_parked();
        if ready(cx) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("native UI deadline exceeded");
}
fn answer(
    cx: &mut TestAppContext,
    result: gpui_kit::Task<Result<RpcValue, WireError>>,
) -> Result<RpcValue, WireError> {
    pump(cx, |_| result.is_ready());
    smol::block_on(result)
}
fn modal(cx: &mut TestAppContext, w: AnyWindowHandle) {
    use gpui_kit::component::WindowExt;
    pump(cx, |cx| {
        cx.update_window(w, |_, w, cx| {
            w.render_frame(cx);
            w.has_active_dialog(cx) && w.find_all("client-confirm").len() == 1
        })
        .unwrap()
    });
}
fn spec() -> Value {
    json!({"title":"Native confirmation","message":"Choose a response","detail":"The backend owns button indices","buttons":["Delete","Keep","Inspect"],"defaultId":2,"cancelId":1})
}
#[gpui_kit::test]
fn confirmation_click_preserves_original_button_index(cx: &mut TestAppContext) {
    let (caps, w) = setup(cx);
    let result = call(cx, &caps, "client:confirmDialog", spec());
    cx.run_until_parked();
    modal(cx, w);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click(("client-confirm-button", 1usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"response":1})
    );
}
#[gpui_kit::test]
fn default_button_and_escape_use_backend_ids(cx: &mut TestAppContext) {
    let (caps, w) = setup(cx);
    let result = call(cx, &caps, "client:confirmDialog", spec());
    cx.run_until_parked();
    modal(cx, w);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"response":2})
    );
    // Let the foreground modal lease observe completion before another request.
    std::thread::sleep(std::time::Duration::from_millis(30));
    cx.run_until_parked();
    let result = call(cx, &caps, "client:confirmDialog", spec());
    cx.run_until_parked();
    modal(cx, w);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"response":1})
    );
}
#[gpui_kit::test]
fn file_picker_maps_directories_multiselect_and_cancellation(cx: &mut TestAppContext) {
    let (caps, _w) = setup(cx);
    let result = call(
        cx,
        &caps,
        "client:openFileDialog",
        json!({"title":"Pick directories","properties":["openDirectory","createDirectory","multiSelections"]}),
    );
    cx.run_until_parked();
    assert!(cx.did_prompt_for_paths());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().to_path_buf();
    cx.simulate_path_prompt_response(|o| {
        assert!(!o.files && o.directories && o.multiple);
        Some(vec![path.clone()])
    });
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"canceled":false,"filePaths":[path]})
    );
    let result = call(cx, &caps, "client:openFileDialog", json!({}));
    cx.run_until_parked();
    cx.simulate_path_prompt_response(|o| {
        assert!(o.files && !o.directories);
        None
    });
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"canceled":true,"filePaths":[]})
    );
}
#[gpui_kit::test]
fn external_url_is_forwarded_to_the_foreground_platform(cx: &mut TestAppContext) {
    let (caps, _) = setup(cx);
    let result = call(
        cx,
        &caps,
        "client:openExternal",
        json!("https://example.com/help"),
    );
    cx.run_until_parked();
    assert_eq!(answer(cx, result).unwrap(), RpcValue::Null);
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/help"));
    let result = call(
        cx,
        &caps,
        "client:openExternal",
        json!("javascript:alert(1)"),
    );
    cx.run_until_parked();
    assert!(answer(cx, result).is_err());
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/help"));
}
#[gpui_kit::test]
fn canceled_requests_close_their_dialog_and_do_not_close_an_unrelated_modal(
    cx: &mut TestAppContext,
) {
    use gpui_kit::component::WindowExt;
    let (caps, w) = setup(cx);
    let pending = call(cx, &caps, "client:confirmDialog", spec());
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.has_active_dialog(cx));
    })
    .unwrap();
    drop(pending);
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(40));
    cx.run_until_parked();
    pump(cx, |cx| {
        cx.update_window(w, |_, w, cx| !w.has_active_dialog(cx))
            .unwrap()
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(!w.has_active_dialog(cx));
        w.open_alert_dialog(cx, |a, _, _| a.title("Application dialog"));
    })
    .unwrap();
    let pending = call(cx, &caps, "client:confirmDialog", spec());
    cx.run_until_parked();
    drop(pending);
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(40));
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.has_active_dialog(cx));
        w.close_dialog(cx);
    })
    .unwrap();
}
#[gpui_kit::test]
fn windowless_complex_confirmation_preserves_duplicate_labels_and_default_index(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let caps = cx.update(|cx| {
        gpui_kit::init(cx);
        NativeCapabilities::install(cx)
    });
    let result = call(
        cx,
        &caps,
        "client:confirmDialog",
        json!({"title":"Windowless","message":"Choose","buttons":["Same","Same","Third","Fourth"],"defaultId":3,"cancelId":0}),
    );
    cx.run_until_parked();
    let window = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 1);
        cx.windows()[0]
    });
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"response":3})
    );
    std::thread::sleep(std::time::Duration::from_millis(40));
    cx.run_until_parked();
    pump(cx, |cx| cx.update(|cx| cx.windows().is_empty()));
}

#[gpui_kit::test]
fn closing_the_temporary_confirmation_window_returns_the_cancel_index(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let caps = cx.update(|cx| {
        gpui_kit::init(cx);
        NativeCapabilities::install(cx)
    });
    let result = call(
        cx,
        &caps,
        "client:confirmDialog",
        json!({"title":"Windowless","message":"Choose","buttons":["A","B","C","D"],"defaultId":3,"cancelId":1}),
    );
    pump(cx, |cx| cx.update(|cx| !cx.windows().is_empty()));
    let window = cx.update(|cx| cx.windows()[0]);
    cx.update_window(window, |_, w, _| w.remove_window())
        .unwrap();
    assert_eq!(
        serde_json::to_value(answer(cx, result).unwrap()).unwrap(),
        json!({"response":1})
    );
}
