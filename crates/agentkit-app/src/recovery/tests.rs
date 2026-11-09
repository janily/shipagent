#![allow(clippy::unwrap_used)]
use super::*;
use ::core::prelude::v1::test;
use gpui_kit::test::TestWindowExt;
fn until(cx: &mut TestAppContext, mut ready: impl FnMut(&mut TestAppContext) -> bool) {
    for _ in 0..2000 {
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
        if ready(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("recovery_contract_deadline");
}
#[::core::prelude::v1::test]
#[ignore = "Requires built backend; explicit native CI contract step"]
fn recovery_contract_parent() {
    let temp = tempfile::tempdir().unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "controller::recovery::tests::recovery_contract_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("AGENTKIT_CONFIG_DIR", temp.path().join("backend"))
        .env("AGENTKIT_APP_DATA_DIR", temp.path().join("client"))
        .env_remove("AGENTKIT_REMOTE_URL")
        .env_remove("AGENTKIT_REMOTE_TOKEN")
        .status()
        .unwrap();
    assert!(result.success());
}
#[gpui_kit::test]
#[ignore = "Isolated subprocess owned by recovery_contract_parent"]
fn recovery_contract_child(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let runtime: RuntimeSlot = Rc::new(RefCell::new(None));
    let quitting = Rc::new(Cell::new(false));
    let (w, controller) = cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::exit::ExitState::default());
        let capabilities = agentkit_capabilities::client_caps::NativeCapabilities::install(cx);
        let preferences = PreferenceStore::new(
            PathBuf::from(std::env::var_os("AGENTKIT_APP_DATA_DIR").unwrap()).join("prefs.json"),
            Preferences {
                onboarding_deferred: true,
                browser_initialized: true,
                ..Default::default()
            },
        );
        let brand = Brand {
            app_name: "Recovery".into(),
            bundle_id: "dev.shipagent.recovery-test".into(),
            data_dir_name: ".shipagent-recovery-test".into(),
            url_scheme: "shipagent-test".into(),
            default_theme: "light".into(),
        };
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                WorkbenchController::new(
                    brand,
                    runtime.clone(),
                    Rc::new(RefCell::new(None)),
                    quitting.clone(),
                    WindowServices {
                        preferences,
                        capabilities,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    until(cx, |cx| {
        cx.update(|cx| {
            let phase = controller.read(cx).view.read(cx).connection_state();
            if matches!(phase, Phase::Fatal { .. }) {
                panic!("{phase:?}");
            }
            controller.read(cx).view.read(cx).connection_ready()
        })
    });
    let (api, id, epoch) = cx.update(|cx| {
        let this = controller.read(cx);
        (
            this.api().unwrap(),
            this.state
                .as_ref()
                .unwrap()
                .read(cx)
                .model
                .state
                .session
                .id
                .clone(),
            this.transport_epoch,
        )
    });
    let workspace = cx.update(|cx| controller.read(cx).workspace.as_ref().unwrap().id.clone());
    let background = smol::block_on(api.create_session_generated(&workspace, None)).unwrap();
    let background_id = background.id.clone();
    cx.update_window(w,|_,window,cx|controller.update(cx,|this,cx|{
        let store=cx.new(|_|SessionStore::new(background));this.stores.insert(background_id.clone(),store);
        this.recovery.pending=true;
        let event=serde_json::from_value(serde_json::json!({"type":"permission_request","sessionId":background_id,"request":{"sessionId":background_id,"requestId":"buffered","toolName":"Bash","description":"fixture"}})).unwrap();
        this.route(vec![(event,reduction_context())],window,cx);
        assert_eq!(this.recovery.events.len(),1);
        this.interrupt_refresh(window,cx);
        assert!(this.recovery.events.is_empty());
        assert_eq!(this.stores[&background_id].read(cx).model.permissions.len(),1);
        // A rapid reconnect can supersede a transaction between monitor ticks.
        this.recovery.pending=true;
        let event=serde_json::from_value(serde_json::json!({"type":"permission_request","sessionId":background_id,"request":{"sessionId":background_id,"requestId":"superseded","toolName":"Bash","description":"fixture"}})).unwrap();
        this.route(vec![(event,reduction_context())],window,cx);
        this.refresh_connection(api.clone(),api.snapshot(),window,cx);
        assert_eq!(this.stores[&background_id].read(cx).model.permissions.len(),2);
    })).unwrap();
    until(cx, |cx| {
        cx.update(|cx| !controller.read(cx).recovery.pending)
    });
    cx.update(|cx| {
        assert_eq!(
            controller.read(cx).stores[&background_id]
                .read(cx)
                .model
                .permissions
                .len(),
            2
        )
    });
    smol::block_on(api.rename_session(&id, "persisted_recovery_name")).unwrap();
    cx.update_window(w, |_, window, cx| {
        controller.update(cx, |this, cx| {
            // A local draft must outlive backend replacement.
            this.view.update(cx, |v, cx| {
                v.restore_draft("unsent_recovery_draft", window, cx)
            });
            this.restart_backend(false, window, cx);
            assert!(!this.view.read(cx).connection_ready());
        });
    })
    .unwrap();
    until(cx, |cx| {
        cx.update(|cx| {
            let this = controller.read(cx);
            this.transport_epoch > epoch && this.view.read(cx).connection_ready()
        })
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        let this = controller.read(cx);
        let session = &this.state.as_ref().unwrap().read(cx).model.state.session;
        assert_eq!(session.id, id);
        assert_eq!(session.name.as_deref(), Some("persisted_recovery_name"));
        assert_eq!(
            this.view.read(cx).current_draft(cx).unwrap().1,
            "unsent_recovery_draft"
        );
        assert_eq!(
            this.list
                .read(cx)
                .model
                .sessions
                .iter()
                .filter(|s| s.id == id)
                .count(),
            1
        );
        assert!(window.try_find("connection-banner").is_none());
    })
    .unwrap();
    // A disconnected OAuth owner must release the wizard, not leave disabled choices.
    cx.update_window(w, |_, window, cx| {
        controller.update(cx, |this, cx| {
            let (tx, rx) = async_channel::bounded(1);
            this.oauth_cancel = Some(tx);
            this.view.update(cx, |v, cx| v.show_onboarding(cx));
            let wizard = this.view.read(cx).onboarding().unwrap();
            wizard.update(cx, |v, cx| {
                v.methods(cx);
                v.oauth_pending(true, cx);
            });
            this.close_recovery_editor(window, cx);
            assert!(rx.is_closed());
        });
        window.render_frame(cx);
        window.click("onboarding-welcome", cx);
        window.render_frame(cx);
        assert!(window.try_find("onboarding-start").is_some());
        controller.update(cx, |this, cx| {
            let (tx, rx) = async_channel::bounded(1);
            this.claude_cancel = Some(tx);
            let wizard = this.view.read(cx).onboarding().unwrap();
            wizard.update(cx, |v, cx| {
                v.methods(cx);
                v.status(true, None, cx);
            });
            this.close_claude(window, cx);
            assert!(rx.is_closed());
            assert!(this.claude_cancel.is_some());
            this.close_recovery_editor(window, cx);
            assert!(this.claude_cancel.is_none());
        });
        window.render_frame(cx);
        window.click("onboarding-welcome", cx);
        window.render_frame(cx);
        assert!(window.try_find("onboarding-start").is_some());
    })
    .unwrap();
    quitting.set(true);
    let running = runtime.borrow_mut().take().unwrap();
    smol::block_on(running.shutdown());
}
