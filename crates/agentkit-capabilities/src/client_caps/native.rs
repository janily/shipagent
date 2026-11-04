use super::{bridge::Request, *};
use agentkit_protocol::RpcValue;
use gpui_kit::AsyncApp;
use std::sync::Arc;
pub(super) async fn execute(
    request: Request,
    recent: Recent,
    modal: Arc<smol::lock::Mutex<()>>,
    cx: &mut AsyncApp,
) {
    if !request.live() {
        return;
    }
    match request.action.clone() {
        Action::OpenExternal(url) => {
            cx.update(|cx| {
                if request.live() {
                    cx.open_url(&url);
                    request.finish(Ok(RpcValue::Null));
                }
            });
        }
        Action::OpenPath(path) => {
            let alive = request.alive.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    if !alive.load(std::sync::atomic::Ordering::Acquire) {
                        return Err(unavailable("Native request canceled"));
                    }
                    Ok(match opener::open(path) {
                        Ok(()) => serde_json::json!({}),
                        Err(e) => serde_json::json!({"error":e.to_string()}),
                    })
                })
                .await;
            request.finish(result.and_then(|v| {
                serde_json::from_value(v).map_err(|_| failure("Invalid native result"))
            }));
        }
        Action::ShowItemInFolder(path) => {
            let alive = request.alive.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    if !alive.load(std::sync::atomic::Ordering::Acquire) {
                        return Err(unavailable("Native request canceled"));
                    }
                    opener::reveal(path)
                        .map(|_| RpcValue::Null)
                        .map_err(|_| failure("Unable to reveal local path"))
                })
                .await;
            request.finish(result);
        }
        Action::Confirm(spec) => {
            let gate = smol::future::race(async { Some(modal.lock().await) }, async {
                let _ = request.canceled.recv().await;
                None
            })
            .await;
            let Some(_gate) = gate else {
                return;
            };
            if !request.live() {
                return;
            }
            let window = cx.update(|cx| parent(&recent, cx));
            match window {
                Some(window) => super::confirm::run(&request, spec, window, cx, false).await,
                None if super::fallback::expressible(&spec) => {
                    let answer = smol::future::race(super::fallback::confirm(&spec), async {
                        let _ = request.canceled.recv().await;
                        Err(unavailable("Native request canceled"))
                    })
                    .await;
                    request.finish(answer);
                }
                None => super::transient::confirm(&request, spec, cx).await,
            }
        }
        Action::OpenFile(spec) => {
            let gate = smol::future::race(async { Some(modal.lock().await) }, async {
                let _ = request.canceled.recv().await;
                None
            })
            .await;
            let Some(_gate) = gate else {
                return;
            };
            if !request.live() {
                return;
            }
            let window = cx.update(|cx| parent(&recent, cx));
            let result = smol::future::race(
                super::files::pick(&spec, window, cx, &request.alive),
                async {
                    let _ = request.canceled.recv().await;
                    Err(unavailable("Native request canceled"))
                },
            )
            .await;
            request.finish(result);
        }
    }
}
