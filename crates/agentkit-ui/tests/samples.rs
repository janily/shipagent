#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_ui::samples::ComponentSamples;
use gpui_kit::{AppContext, TestAppContext, WindowOptions, test::TestWindowExt};

#[gpui_kit::test]
fn samples_submit_chinese_preserve_draft_and_cancel_clear(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let (handle, view) = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ComponentSamples::new(window, cx))
        })
        .unwrap();
        handle
            .update(cx, |_, window, cx| {
                window.render_frame(cx);
                window.click("sample-composer", cx);
                window.input("中文组件验证", cx);
                window.click("sample-send", cx);
                window.render_frame(cx);
                assert_eq!(
                    window.find(("sample-message", 0usize)).label(),
                    Some("中文组件验证")
                );
                window.click("sample-composer", cx);
                window.input("保留草稿", cx);
                window.click("sample-settings", cx);
                window.render_frame(cx);
                assert!(window.try_find("sample-settings-view").is_some());
                window.click("sample-back", cx);
                window.render_frame(cx);
                assert_eq!(
                    window.within("sample-composer").find("input-state").value(),
                    Some("保留草稿")
                );
                window.click("sample-clear", cx);
                window.render_frame(cx);
                window.click("sample-clear-cancel", cx);
                window.render_frame(cx);
                assert_eq!(
                    window.find(("sample-message", 0usize)).label(),
                    Some("中文组件验证")
                );
                assert_eq!(
                    window.within("sample-composer").find("input-state").value(),
                    Some("保留草稿")
                );
                window.click("sample-clear", cx);
                window.render_frame(cx);
                window.click("sample-clear-confirm", cx);
                window.render_frame(cx);
                assert!(window.try_find(("sample-message", 0usize)).is_none());
                assert_eq!(
                    window.within("sample-composer").find("input-state").value(),
                    Some("")
                );
                // Empty submission keeps the initial empty-state visible.
                window.click("sample-send", cx);
                window.render_frame(cx);
                assert!(window.try_find("sample-empty").is_some());
                window.click("sample-composer", cx);
                window.input("   ", cx);
                window.click("sample-send", cx);
                window.render_frame(cx);
                assert_eq!(
                    window.within("sample-composer").find("input-state").value(),
                    Some("   ")
                );
                assert!(window.try_find("sample-empty").is_some());
                let _ = view;
            })
            .unwrap();
    });
}

#[gpui_kit::test]
fn sample_collapsible_hides_and_reveals_tool_details(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let (handle, _) = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ComponentSamples::new(window, cx))
        })
        .unwrap();
        handle
            .update(cx, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("sample-tool-result").is_none());
                window.click("sample-tool-toggle", cx);
                window.render_frame(cx);
                assert!(window.try_find("sample-tool-result").is_some());
                window.click("sample-tool-toggle", cx);
                window.render_frame(cx);
                assert!(window.try_find("sample-tool-result").is_none());
            })
            .unwrap();
    });
}

#[gpui_kit::test]
fn minimum_window_keeps_input_visible_and_panels_resizable(cx: &mut TestAppContext) {
    use gpui_kit::{WindowBounds, px, size};
    cx.update(|cx| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(960.), px(600.)), cx)),
            ..WindowOptions::default()
        };
        let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| ComponentSamples::new(window, cx))
        })
        .unwrap();
        handle
            .update(cx, |_, window, cx| {
                window.render_frame(cx);
                window.render_frame(cx);
                let send = window.find("sample-send").bounds();
                assert!(send.bottom() < px(600.));
                assert!(send.top() > px(400.));
                let inspector = window.find("sample-inspector").bounds();
                assert!(inspector.size.height > px(400.));
                let handles = window.find_all(("resizable-handle", 0usize));
                let from = handles.last().unwrap().bounds().center();
                window.drag(from, from + gpui_kit::point(px(-60.), px(0.)), cx);
                window.render_frame(cx);
                let resized = window.find("sample-inspector").bounds();
                assert!(resized.size.width > inspector.size.width);
                assert!(resized.size.width <= px(360.));
                assert!(window.find("sample-send").bounds().right() <= resized.left());
            })
            .unwrap();
    });
}
