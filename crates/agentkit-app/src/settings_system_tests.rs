#![allow(clippy::unwrap_used)]
use super::*;
use ::core::prelude::v1::test;
use agentkit_ui::recovery::Phase;
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
    panic!("settings_contract_deadline");
}
#[::core::prelude::v1::test]
#[ignore = "Requires built backend; explicit native settings CI contract step"]
fn settings_contract_parent() {
    let temp = tempfile::tempdir().unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "controller::settings_actions::system::tests::settings_contract_child",
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
#[ignore = "Isolated subprocess owned by settings_contract_parent"]
fn settings_contract_child(cx: &mut TestAppContext) {
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
    let (api, workspace, token) = cx.update(|cx| {
        let this = controller.read(cx);
        (
            this.api().unwrap(),
            this.workspace.as_ref().unwrap().id.clone(),
            this.runtime
                .borrow()
                .as_ref()
                .unwrap()
                .sidecar
                .as_ref()
                .unwrap()
                .endpoint()
                .token
                .expose_secret()
                .to_string(),
        )
    });
    let act = |cx: &mut TestAppContext, action: SettingsIntent| {
        cx.update_window(w, |_, window, cx| {
            controller.update(cx, |this, cx| this.handle_settings(action, window, cx))
        })
        .unwrap();
    };
    cx.update_window(w, |_, window, cx| {
        controller.update(cx, |this, cx| {
            this.view
                .update(cx, |v, cx| v.show_configuration(window, cx))
        })
    })
    .unwrap();
    act(cx, SettingsIntent::Reload);
    until(cx, |cx| {
        cx.update(|cx| !controller.read(cx).settings_pending)
    });
    act(cx, SettingsIntent::KeepAwake(true));
    until(cx, |cx| {
        cx.update(|cx| !controller.read(cx).settings_pending)
    });
    assert!(smol::block_on(api.keep_awake()).unwrap());

    // Permission defaults must not change before the explicit execution-mode confirmation.
    let original = smol::block_on(api.workspace_settings(&workspace)).unwrap();
    act(
        cx,
        SettingsIntent::PermissionMode(
            workspace.clone(),
            agentkit_protocol::generated::PermissionMode::AllowAll,
        ),
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        serde_json::to_value(smol::block_on(api.workspace_settings(&workspace)).unwrap()).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    act(
        cx,
        SettingsIntent::PermissionMode(
            workspace.clone(),
            agentkit_protocol::generated::PermissionMode::AllowAll,
        ),
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("ok", cx);
    })
    .unwrap();
    until(cx, |cx| {
        cx.update(|cx| !controller.read(cx).settings_pending)
    });
    assert_eq!(
        smol::block_on(api.workspace_settings(&workspace))
            .unwrap()
            .unwrap()
            .permission_mode,
        Some(agentkit_protocol::generated::PermissionMode::AllowAll)
    );
    act(
        cx,
        SettingsIntent::PermissionMode(
            workspace.clone(),
            agentkit_protocol::generated::PermissionMode::Safe,
        ),
    );
    until(cx, |cx| {
        cx.update(|cx| !controller.read(cx).settings_pending)
    });

    let values = || ProxyValues {
        enabled: false,
        http_proxy: "http://user:fixture-secret@127.0.0.1:9876".into(),
        ..Default::default()
    };
    let previous_epoch = cx.update(|cx| controller.read(cx).transport_epoch);
    cx.update_window(w, |_, window, cx| {
        controller.update(cx, |this, cx| {
            this.view.update(cx, |v, cx| {
                v.restore_draft("settings-retained-draft", window, cx)
            })
        })
    })
    .unwrap();
    act(cx, SettingsIntent::Proxy(values()));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!smol::block_on(api.network_proxy()).unwrap().enabled);
    assert!(
        smol::block_on(api.network_proxy())
            .unwrap()
            .http_proxy
            .is_none()
    );
    assert_eq!(
        cx.update(|cx| controller.read(cx).transport_epoch),
        previous_epoch
    );

    // A failed backend save cannot restart or disclose the proxy credential.
    let config =
        PathBuf::from(std::env::var_os("AGENTKIT_CONFIG_DIR").unwrap()).join("config.json");
    let original = std::fs::read(&config).unwrap();
    std::fs::write(&config, br#"{"workspaces":null}"#).unwrap();
    act(cx, SettingsIntent::Proxy(values()));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("ok", cx);
    })
    .unwrap();
    until(cx, |cx| {
        cx.update(|cx| !controller.read(cx).settings_pending)
    });
    assert_eq!(
        cx.update(|cx| controller.read(cx).transport_epoch),
        previous_epoch
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        let error = window.find("settings-error");
        assert!(!error.label().unwrap().contains("fixture-secret"));
        assert_eq!(
            error.label(),
            Some(agentkit_ui::t!("ak.settings.proxy_failed").as_str())
        );
    })
    .unwrap();
    std::fs::write(&config, original).unwrap();

    act(cx, SettingsIntent::Proxy(values()));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("ok", cx);
    })
    .unwrap();
    until(cx, |cx| {
        cx.update(|cx| {
            let this = controller.read(cx);
            this.transport_epoch > previous_epoch
                && this.view.read(cx).connection_ready()
                && !this.settings_pending
        })
    });
    let (next, next_token) = cx.update(|cx| {
        let this = controller.read(cx);
        assert_eq!(
            this.view.read(cx).current_draft(cx).unwrap().1,
            "settings-retained-draft"
        );
        (
            this.api().unwrap(),
            this.runtime
                .borrow()
                .as_ref()
                .unwrap()
                .sidecar
                .as_ref()
                .unwrap()
                .endpoint()
                .token
                .expose_secret()
                .to_string(),
        )
    });
    assert_ne!(token, next_token);
    let saved = smol::block_on(next.network_proxy()).unwrap();
    assert!(!saved.enabled);
    assert_eq!(
        saved.http_proxy.unwrap().expose_secret(),
        "http://user:fixture-secret@127.0.0.1:9876"
    );
    assert!(smol::block_on(next.keep_awake()).unwrap());
    assert_eq!(
        smol::block_on(next.workspace_settings(&workspace))
            .unwrap()
            .unwrap()
            .permission_mode,
        Some(agentkit_protocol::generated::PermissionMode::Safe)
    );
    act(
        cx,
        SettingsIntent::Local(LocalChange::CompletionNotifications(false)),
    );
    act(
        cx,
        SettingsIntent::Local(LocalChange::ApprovalNotifications(false)),
    );
    act(
        cx,
        SettingsIntent::Local(LocalChange::SendKey(
            agentkit_ui::composer::SendKey::CmdEnter,
        )),
    );
    let preferences = cx.update(|cx| controller.read(cx).preferences.clone());
    smol::block_on(preferences.finish()).unwrap();
    let restored = crate::preferences::load(
        &PathBuf::from(std::env::var_os("AGENTKIT_APP_DATA_DIR").unwrap()).join("prefs.json"),
        Preferences::default(),
    )
    .unwrap();
    assert!(!restored.completion_notifications && !restored.approval_notifications);
    assert_eq!(restored.send_key, agentkit_ui::composer::SendKey::CmdEnter);
    quitting.set(true);
    let running = runtime.borrow_mut().take().unwrap();
    smol::block_on(running.shutdown());
}
