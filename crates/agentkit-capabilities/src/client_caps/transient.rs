use super::{bridge::Request, *};
use gpui_kit::{prelude::*, *};
struct Host;
impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}
/// rfd cannot represent arbitrary counts/duplicate labels/default IDs. Keep the exact
/// contract using an owned temporary Kit window rather than truncating/reordering choices.
pub(super) async fn confirm(request: &Request, spec: ConfirmSpec, cx: &mut AsyncApp) {
    if !request.live() {
        return;
    }
    let title = spec.title.clone();
    let window = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(640.), px(360.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| {
                window.set_window_title(&title);
                cx.new(|_| Host)
            },
        )
    });
    let Ok((handle, _)) = window else {
        request.finish(Err(unavailable("No native dialog host available")));
        return;
    };
    super::confirm::run(request, spec, handle, cx, true).await;
    let _ = handle.update(cx, |_, window, _| window.remove_window());
}
