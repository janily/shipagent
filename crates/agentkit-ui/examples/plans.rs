//! Explicit local PlanCard visual fixture; no RPC, approval or provider request.
use agentkit_store::entities::SessionStore;
use agentkit_ui::transcript::PlanCard;
use gpui_kit::{
    component::{ActiveTheme, Theme, ThemeMode, TitleBar},
    prelude::*,
    *,
};
struct Preview(Entity<PlanCard>);
impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .p_8()
            .child(div().max_w(px(820.)).mx_auto().child(self.0.clone()))
    }
}
fn main() {
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx|{
        gpui_kit::init(cx);agentkit_ui::set_locale("zh-CN");
        if std::env::var_os("AGENTKIT_PLAN_DARK").is_some(){Theme::change(ThemeMode::Dark,None,cx);}
        cx.on_window_closed(|cx,_|if cx.windows().is_empty(){cx.quit();}).detach();
        let session=serde_json::from_value(serde_json::json!({"id":"visual-fixture","workspaceId":"fixture","workspaceName":"fixture","messages":[{"id":"plan","role":"plan","content":"/visual-fixture/plan.md","timestamp":1}],"lastMessageAt":1,"isProcessing":false}));
        let Ok(session)=session else{cx.quit();return;};
        let options=WindowOptions{window_bounds:Some(WindowBounds::centered(size(px(1280.),px(820.)),cx)),..TitleBar::window_options()};
        let _=gpui_kit::open_window(options,cx,|_,cx|{
            let store=cx.new(|_|{let mut s=SessionStore::new(session);s.model.set_draft(include_str!("fixtures/plan-draft.txt").into());s.plan.content.insert("plan".into(),Ok(include_str!("fixtures/plan-preview.md").into()));s});
            let card=cx.new(|cx|PlanCard::new(store,"plan".into(),cx));cx.new(|_|Preview(card))
        });cx.activate(true);
    });
}
