//! Opt-in native confirmation fixture, using the pinned real transport and production handlers.
use agentkit_capabilities::client_caps::NativeCapabilities;
use agentkit_client::{ClientOptions, RpcClient};
use gpui_kit::component::{TitleBar, WindowExt};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
struct View;
impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::component::v_flex()
            .size_full()
            .child(TitleBar::new().child("Native capability verification"))
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("Expected new report path")?);
    let stages = report.clone();
    let output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report)?;
    let endpoint = std::env::var("AGENTKIT_CAP_SMOKE_URL")?;
    let token = std::env::var("SHIPAGENT_CONTRACT_TOKEN")?;
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        let caps=NativeCapabilities::install(cx);
        let opened=gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:Point::default(),size:size(px(1000.),px(700.))})),..TitleBar::window_options()},cx,|w,cx| {w.set_window_title("Native capability verification");cx.new(|_|View)});
        let Ok((window,_))=opened else {cx.quit();return;};caps.remember(window);
        let mut options=ClientOptions::local(endpoint,token.into(),"native-smoke".into());options.capabilities=caps.handlers();
        let client=RpcClient::connect(options);
        cx.spawn(async move |cx| {
            let _keep=caps;
            let result:Result<Vec<Value>,agentkit_client::RpcError>=async {
                let spec=json!({"title":"原生客户端能力验证","message":"按 Enter 选择默认的第三个按钮。","detail":"该请求来自固定上游 transport 的反向 RPC。","buttons":["删除","保留","检查"],"defaultId":2,"cancelId":1,"type":"question"});
                let first=native_call(&client,window,spec,stages.with_extension("stage1"),cx).await?;
                let spec=json!({"title":"原生取消验证","message":"按 Escape 返回后端指定的取消序号。","buttons":["允许","取消"],"defaultId":0,"cancelId":1});
                let second=native_call(&client,window,spec,stages.with_extension("stage2"),cx).await?;
                Ok(vec![first,second])
            }.await;
            client.close();
            let value=match result {Ok(values)=>json!({"status":if values==[json!({"response":2}),json!({"response":1})]{"passed"}else{"failed"},"responses":values,"physicalDesktopAcceptance":"not_evaluated"}),Err(e)=>json!({"status":"failed","error":e.protocol_code()})};
            let _=cx.background_executor().spawn(async move {serde_json::to_writer(output,&value)}).await;
            cx.update(|cx|cx.quit());
        }).detach();
        cx.activate(true);
    });
    Ok(())
}

async fn native_call(
    client: &RpcClient,
    window: AnyWindowHandle,
    spec: Value,
    stage: std::path::PathBuf,
    cx: &mut AsyncApp,
) -> Result<Value, agentkit_client::RpcError> {
    let client = client.clone();
    let task = cx.background_executor().spawn(async move {
        client
            .invoke(
                "contract:reverse",
                vec![json!("client:confirmDialog"), spec],
            )
            .await
    });
    let mut announced = false;
    while !task.is_ready() {
        let showing = window
            .update(cx, |_, window, cx| window.has_active_dialog(cx))
            .unwrap_or(false);
        if showing && !announced {
            announced = true;
            let stage = stage.clone();
            let _ = cx
                .background_executor()
                .spawn(async move { std::fs::write(stage, b"ready") })
                .await;
        }
        cx.background_executor()
            .timer(std::time::Duration::from_millis(20))
            .await;
    }
    task.await
}
