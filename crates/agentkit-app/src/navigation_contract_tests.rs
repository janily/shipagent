#[cfg(test)]
mod contract {
    #![allow(clippy::unwrap_used)]
    use super::super::*;
    use ::core::prelude::v1::test;
    use gpui_kit::test::TestWindowExt;
    fn until(
        cx: &mut TestAppContext,
        phase: &str,
        mut ready: impl FnMut(&mut TestAppContext) -> bool,
    ) {
        for _ in 0..3000 {
            cx.executor().advance_clock(Duration::from_millis(10));
            cx.run_until_parked();
            if ready(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("navigation deadline: {phase}");
    }
    struct Model(std::process::Child);
    impl Drop for Model {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    #[test]
    #[ignore = "Requires bundled backend and Node; native navigation contract"]
    fn navigation_contract_parent() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
        };
        smol::block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let config = temp.path().join("backend");
            let frontend = temp.path().join("client");
            let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let backend = PathBuf::from(std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap());
            let sidecar = agentkit_sidecar::SidecarHandle::start(
                agentkit_sidecar::SidecarConfig::new(backend.clone(), config.clone()),
            )
            .await
            .unwrap();
            let initial = WorkbenchApi::connect(sidecar.endpoint(), String::new());
            let workspace = initial.workspaces_generated().await.unwrap().remove(0);
            let root = PathBuf::from(&workspace.root_path);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(
                root.join("plan.md"),
                "# Navigation plan\n1. Verify filters.\n",
            )
            .unwrap();
            let mut model = Model(
                Command::new("node")
                    .arg(repo.join("tools/fixtures/model.mjs"))
                    .arg(repo.join("tools/fake-llm/scenarios/plan.yaml"))
                    .env("AGENTKIT_FIXTURE_ROOT", &root)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap(),
            );
            let out = model.0.stdout.take().unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut s = String::new();
                BufReader::new(out).read_line(&mut s).unwrap();
                tx.send(s).unwrap();
            });
            let url = rx.recv_timeout(Duration::from_secs(10)).unwrap();
            initial
                .setup_endpoint(
                    url.trim().strip_prefix("FAKE_LLM_URL=").unwrap(),
                    "fixture-model",
                    None,
                )
                .await
                .unwrap();
            initial.disable_browser().await.unwrap();
            initial.close();
            let api = WorkbenchApi::connect(sidecar.endpoint(), workspace.id.clone());
            let a = api
                .create_session_generated(&workspace.id, Some(&workspace.root_path))
                .await
                .unwrap();
            let b = api
                .create_session_generated(&workspace.id, Some(&workspace.root_path))
                .await
                .unwrap();
            api.rename_session(&a.id, "Title target").await.unwrap();
            assert!(
                api.send(&a.id, "ZEPHYR_SEARCH_PROOF prepare a plan.", "nav-seed")
                    .await
                    .unwrap()
                    .accepted
            );
            let deadline = std::time::Instant::now() + Duration::from_secs(40);
            loop {
                let s = api.session_generated(&a.id).await.unwrap();
                if !s.is_processing && agentkit_store::core::views::pending_review(&s) {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                smol::Timer::after(Duration::from_millis(20)).await;
            }
            crate::test_persistence::durable_bundle(sidecar.endpoint(), &workspace.id, &a.id)
                .await
                .unwrap();
            std::fs::create_dir_all(&frontend).unwrap();
            std::fs::write(frontend.join("prefs.json"),serde_json::to_vec(&serde_json::json!({"version":1,"language":"en","browser_initialized":true,"workspace":workspace.id,"session":b.id})).unwrap()).unwrap();
            api.close();
            sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
            let result = Command::new(std::env::current_exe().unwrap())
                .args([
                    "controller::navigation::contract_tests::contract::navigation_contract_child",
                    "--exact",
                    "--ignored",
                    "--nocapture",
                ])
                .env("AGENTKIT_CONFIG_DIR", &config)
                .env("AGENTKIT_APP_DATA_DIR", &frontend)
                .env("AGENTKIT_NAV_A", &a.id)
                .env("AGENTKIT_NAV_B", &b.id)
                .env_remove("AGENTKIT_REMOTE_URL")
                .env_remove("AGENTKIT_REMOTE_TOKEN")
                .status()
                .unwrap();
            assert!(result.success());
            let sidecar = agentkit_sidecar::SidecarHandle::start(
                agentkit_sidecar::SidecarConfig::new(backend, config),
            )
            .await
            .unwrap();
            let api = WorkbenchApi::connect(sidecar.endpoint(), workspace.id);
            let a = api.session_generated(&a.id).await.unwrap();
            assert_eq!(a.is_flagged, Some(true));
            assert_eq!(a.is_archived, Some(true));
            assert_eq!(a.has_unread, Some(true));
            api.close();
            sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
        });
    }
    #[gpui_kit::test]
    #[ignore = "Isolated subprocess owned by navigation_contract_parent"]
    fn navigation_contract_child(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let a = std::env::var("AGENTKIT_NAV_A").unwrap();
        let b = std::env::var("AGENTKIT_NAV_B").unwrap();
        let runtime: RuntimeSlot = Rc::new(RefCell::new(None));
        let quitting = Rc::new(Cell::new(false));
        let (w, controller) = cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(crate::exit::ExitState::default());
            agentkit_ui::set_locale("en");
            let capabilities = agentkit_capabilities::client_caps::NativeCapabilities::install(cx);
            let prefs = PathBuf::from(std::env::var_os("AGENTKIT_APP_DATA_DIR").unwrap())
                .join("prefs.json");
            let preferences = PreferenceStore::new(
                prefs.clone(),
                crate::preferences::load(&prefs, Preferences::default()).unwrap(),
            );
            let brand = Brand {
                app_name: "Navigation".into(),
                bundle_id: "dev.shipagent.navigation-test".into(),
                data_dir_name: ".shipagent-navigation-test".into(),
                url_scheme: "shipagent-navigation-test".into(),
                default_theme: "light".into(),
            };
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(1280.), px(820.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
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
                },
            )
            .unwrap()
        });
        until(cx, "cold plan index", |cx| {
            cx.update(|cx| {
                let this = controller.read(cx);
                this.view.read(cx).connection_ready()
                    && this.list.read(cx).model.plans.contains(&a)
                    && !this.list.read(cx).plan_loading
            })
        });
        cx.update_window(w, |_, window, cx| {
            controller.update(cx, |this, cx| {
                this.handle(WorkbenchIntent::OpenSession(b.clone()), window, cx)
            })
        })
        .unwrap();
        until(cx, "open second session", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .state
                    .as_ref()
                    .is_some_and(|s| s.read(cx).model.state.session.id == b)
            })
        });
        let (workspace, endpoint) = cx.update(|cx| {
            let this = controller.read(cx);
            (
                this.workspace.as_ref().unwrap().id.clone(),
                this.runtime
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .sidecar
                    .as_ref()
                    .unwrap()
                    .endpoint(),
            )
        });
        let other = WorkbenchApi::connect(endpoint, workspace.clone());
        let events = other.session_events();
        smol::block_on(other.workspaces_generated()).unwrap();
        let plan_path = smol::block_on(other.session_generated(&a))
            .unwrap()
            .messages
            .into_iter()
            .find(|m| m.role == agentkit_protocol::generated::MessageRole::Plan)
            .unwrap()
            .content;
        let version = cx.update(|cx| {
            let this = controller.read(cx);
            let s = this
                .list
                .read(cx)
                .model
                .sessions
                .iter()
                .find(|s| s.id == a)
                .unwrap();
            assert!(!this.list.read(cx).model.plans.contains(&b));
            (s.last_message_at, s.is_processing)
        });
        smol::block_on(other.prepare_plan(&a, &plan_path, "")).unwrap();
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click("view:4", cx);
        })
        .unwrap();
        until(cx, "explicit pending review refresh", |cx| {
            cx.update(|cx| {
                let this = controller.read(cx);
                !this.list.read(cx).plan_loading && !this.list.read(cx).model.plans.contains(&a)
            })
        });
        smol::block_on(other.clear_plan(&a)).unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
        until(cx, "periodic pending clear refresh", |cx| {
            cx.update(|cx| controller.read(cx).list.read(cx).model.plans.contains(&a))
        });
        cx.update(|cx| {
            let s = controller
                .read(cx)
                .list
                .read(cx)
                .model
                .sessions
                .iter()
                .find(|s| s.id == a)
                .unwrap();
            assert_eq!((s.last_message_at, s.is_processing), version);
        });
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click("view:0", cx);
        })
        .unwrap();
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click(format!("session-menu:{a}"), cx);
            window.within("popup-menu").click(0usize, cx);
        })
        .unwrap();
        until(cx, "flag native command", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.id == a && s.is_flagged == Some(true))
            })
        });
        let received = smol::block_on(async {
            smol::future::race(
                async {
                    loop {
                        let batch = events.recv().await.unwrap();
                        if batch.iter().any(|e| e["type"] == "session_flagged") {
                            break true;
                        }
                    }
                },
                async {
                    smol::Timer::after(Duration::from_secs(5)).await;
                    false
                },
            )
            .await
        });
        assert!(received);
        smol::block_on(other.set_session_flag(&a, false)).unwrap();
        until(cx, "flag broadcast", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.id == a && s.is_flagged == Some(false))
            })
        });
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click(format!("session-menu:{a}"), cx);
            window.within("popup-menu").click(2usize, cx);
        })
        .unwrap();
        until(cx, "archive", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.id == a && s.is_archived == Some(true))
            })
        });
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click("view:5", cx);
            window.render_frame(cx);
            assert!(window.try_find(format!("session:{a}")).is_some());
            window.click(format!("session-menu:{a}"), cx);
            window.within("popup-menu").click(2usize, cx);
        })
        .unwrap();
        until(cx, "unarchive", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.id == a && s.is_archived == Some(false))
            })
        });
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click("view:0", cx);
            window.render_frame(cx);
            window.click(format!("session-menu:{a}"), cx);
            window.within("popup-menu").click(1usize, cx);
        })
        .unwrap();
        until(cx, "mark unread", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.id == a && s.has_unread == Some(true))
            })
        });
        smol::block_on(other.set_session_read(&a, true)).unwrap();
        until(cx, "read broadcast", |cx| {
            cx.update(|cx| {
                controller
                    .read(cx)
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.id == a && s.has_unread == Some(false))
            })
        });
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            window.click("session-search", cx);
            window.input("ZEPHYR_SEARCH_PROOF", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(299));
        cx.run_until_parked();
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(format!("content-session:{a}")).is_none());
        })
        .unwrap();
        until(cx, "body search", |cx| {
            cx.update_window(w, |_, window, cx| {
                window.render_frame(cx);
                window.try_find(format!("content-session:{a}")).is_some()
            })
            .unwrap()
        });
        cx.update_window(w, |_, window, cx| {
            window.click(format!("content-session:{a}"), cx);
        })
        .unwrap();
        until(cx, "jump after load", |cx| {
            cx.update(|cx| {
                let this = controller.read(cx);
                this.state
                    .as_ref()
                    .is_some_and(|s| s.read(cx).model.state.session.id == a)
                    && this.navigation_state.jump.is_none()
                    && !this.loading
            })
        });
        cx.update(|cx| {
            controller.read(cx).view.clone().update(cx, |v, cx| {
                assert!(v.jump_to_match(&a, "ZEPHYR_SEARCH_PROOF", cx))
            })
        });
        cx.update_window(w, |_, window, cx| {
            window.click("session-search", cx);
            window.press("escape", cx);
        })
        .unwrap();
        other.close();
        let before = cx.update(|cx| controller.read(cx).epoch);
        cx.update_window(w, |_, window, cx| {
            controller.update(cx, |this, cx| {
                this.handle(WorkbenchIntent::RestartBackend, window, cx)
            })
        })
        .unwrap();
        until(cx, "restart plan index", |cx| {
            cx.update(|cx| {
                let this = controller.read(cx);
                this.epoch > before
                    && this.view.read(cx).connection_ready()
                    && !this.list.read(cx).plan_loading
                    && this.list.read(cx).model.plans.contains(&a)
            })
        });
        let api = cx.update(|cx| controller.read(cx).api().unwrap());
        smol::block_on(async {
            api.set_session_flag(&a, true).await.unwrap();
            api.set_session_archived(&a, true).await.unwrap();
            api.set_session_read(&a, false).await.unwrap();
            let endpoint = runtime
                .borrow()
                .as_ref()
                .unwrap()
                .sidecar
                .as_ref()
                .unwrap()
                .endpoint();
            crate::test_persistence::durable_bundle(endpoint, &workspace, &a)
                .await
                .unwrap();
        });
        quitting.set(true);
        api.close();
        let running = runtime.borrow_mut().take().unwrap();
        assert!(smol::block_on(running.shutdown()));
    }
}
