#![allow(clippy::unwrap_used)]
//! Real pinned upstream transport, with capability bodies dispatched on the GPUI foreground.
use agentkit_capabilities::client_caps::NativeCapabilities;
use agentkit_client::{ClientOptions, RpcClient};
use gpui_kit::{
    AppContext, Context, IntoElement, Render, Task, TestAppContext, Window, WindowOptions, div,
    test::TestWindowExt,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Fixture {
    child: Child,
    _profile: tempfile::TempDir,
    url: String,
    token: String,
}
impl Fixture {
    fn start() -> Self {
        let backend = std::path::PathBuf::from(std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap());
        let script = std::env::var_os("AGENTKIT_CONTRACT_SERVER").unwrap();
        let profile = tempfile::tempdir().unwrap();
        let token = uuid::Uuid::new_v4().to_string();
        let bun = backend
            .join("vendor/bun")
            .join(if cfg!(windows) { "bun.exe" } else { "bun" });
        let child = Command::new(bun)
            .arg(script)
            .env("CONFIG_DIR", profile.path())
            .env("SHIPAGENT_CONTRACT_TOKEN", &token)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut fixture = Self {
            child,
            _profile: profile,
            url: String::new(),
            token,
        };
        let stdout = fixture.child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(url) = line.strip_prefix("CONTRACT_URL=") {
                    let _ = tx.send(url.to_owned());
                }
            }
        });
        fixture.url = rx.recv_timeout(Duration::from_secs(20)).unwrap();
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct Empty;
impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
fn until(cx: &mut TestAppContext, mut ready: impl FnMut(&mut TestAppContext) -> bool) {
    for _ in 0..1500 {
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
        if ready(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("native reverse RPC deadline exceeded");
}
fn reverse(
    cx: &TestAppContext,
    client: &RpcClient,
    channel: &str,
    value: Value,
) -> Task<Result<Value, agentkit_client::RpcError>> {
    let client = client.clone();
    let channel = channel.to_string();
    cx.executor().spawn(async move {
        client
            .invoke("contract:reverse", vec![json!(channel), value])
            .await
    })
}
#[gpui_kit::test]
#[ignore = "Requires built backend/contract fixture; explicit native CI contract step"]
fn real_reverse_rpc_uses_native_confirmation_picker_and_url_dispatch(cx: &mut TestAppContext) {
    // Real sockets deliberately use the scheduler's supported nondeterministic mode.
    cx.executor().allow_parking();
    let fixture = Fixture::start();
    let (caps, w) = cx.update(|cx| {
        gpui_kit::init(cx);
        let caps = NativeCapabilities::install(cx);
        let (w, _) =
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Empty)).unwrap();
        caps.remember(w);
        (caps, w)
    });
    let mut options = ClientOptions::local(
        fixture.url.clone(),
        fixture.token.clone().into(),
        "native-caps".into(),
    );
    options.capabilities = caps.handlers();
    let client = RpcClient::connect(options);
    let request = reverse(
        cx,
        &client,
        "client:confirmDialog",
        json!({"title":"Real server request","message":"Select an action","buttons":["Accept","Cancel","Inspect"],"defaultId":2,"cancelId":1}),
    );
    until(cx, |cx| {
        cx.update_window(w, |_, w, cx| {
            w.render_frame(cx);
            w.try_find(("client-confirm-button", 2usize))
                .is_some_and(|b| b.visible())
        })
        .unwrap()
    });
    cx.update_window(w, |_, w, cx| w.click(("client-confirm-button", 2usize), cx))
        .unwrap();
    until(cx, |_| request.is_ready());
    assert_eq!(smol::block_on(request).unwrap(), json!({"response":2}));
    std::thread::sleep(Duration::from_millis(40));
    cx.run_until_parked();
    let request = reverse(
        cx,
        &client,
        "client:openFileDialog",
        json!({"properties":["openDirectory","createDirectory"]}),
    );
    until(cx, |cx| cx.did_prompt_for_paths());
    let path = fixture._profile.path().to_path_buf();
    cx.simulate_path_prompt_response(|o| {
        assert!(o.directories && !o.files);
        Some(vec![path.clone()])
    });
    until(cx, |_| request.is_ready());
    assert_eq!(
        smol::block_on(request).unwrap(),
        json!({"canceled":false,"filePaths":[path]})
    );
    let request = reverse(
        cx,
        &client,
        "client:openExternal",
        json!("https://example.com/native-proof"),
    );
    until(cx, |_| request.is_ready());
    assert_eq!(smol::block_on(request).unwrap(), Value::Null);
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://example.com/native-proof")
    );
    for channel in ["client:openPath", "client:showItemInFolder"] {
        let request = reverse(cx, &client, channel, json!("relative/path"));
        until(cx, |_| request.is_ready());
        assert_eq!(
            smol::block_on(request).unwrap_err().protocol_code(),
            "HANDLER_ERROR"
        );
    }
    let request = reverse(cx, &client, "client:browser:invoke", json!({}));
    until(cx, |_| request.is_ready());
    assert_eq!(
        smol::block_on(request).unwrap_err().protocol_code(),
        "CAPABILITY_UNAVAILABLE"
    );
    let pending = reverse(
        cx,
        &client,
        "client:confirmDialog",
        json!({"title":"Disconnect","message":"Pending confirmation","buttons":["Accept","Cancel"],"cancelId":1}),
    );
    until(cx, |cx| {
        cx.update_window(w, |_, w, cx| {
            use gpui_kit::component::WindowExt;
            w.render_frame(cx);
            w.has_active_dialog(cx)
        })
        .unwrap()
    });
    client.close();
    until(cx, |cx| {
        cx.update_window(w, |_, w, cx| {
            use gpui_kit::component::WindowExt;
            !w.has_active_dialog(cx)
        })
        .unwrap()
    });
    until(cx, |_| pending.is_ready());
    assert!(smol::block_on(pending).is_err());
}
