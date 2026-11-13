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
        for _ in 0..2000 {
            cx.executor().advance_clock(Duration::from_millis(10));
            cx.run_until_parked();
            if ready(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("draft contract deadline phase {phase}");
    }
    struct Model(std::process::Child);
    impl Drop for Model {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    #[test]
    #[ignore = "Requires bundled backend and Node; two isolated native client processes"]
    fn draft_contract_parent() {
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
            let mut model = Model(
                Command::new("node")
                    .arg(repo.join("tools/fixtures/model.mjs"))
                    .arg(repo.join("tools/fake-llm/scenarios/text.yaml"))
                    .env("AGENTKIT_FIXTURE_ROOT", temp.path())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap(),
            );
            let stdout = model.0.stdout.take().unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut s = String::new();
                BufReader::new(stdout).read_line(&mut s).unwrap();
                tx.send(s).unwrap();
            });
            let url = rx.recv_timeout(Duration::from_secs(10)).unwrap();
            let sidecar = agentkit_sidecar::SidecarHandle::start(
                agentkit_sidecar::SidecarConfig::new(backend, config.clone()),
            )
            .await
            .unwrap();
            let api = WorkbenchApi::connect(sidecar.endpoint(), String::new());
            let workspace = api.workspaces_generated().await.unwrap().remove(0);
            api.setup_endpoint(
                url.trim().strip_prefix("FAKE_LLM_URL=").unwrap(),
                "fixture-model",
                None,
            )
            .await
            .unwrap();
            api.disable_browser().await.unwrap();
            api.close();
            let api = WorkbenchApi::connect(sidecar.endpoint(), workspace.id.clone());
            let session = api
                .create_session_generated(&workspace.id, Some(&workspace.root_path))
                .await
                .unwrap();
            let skill = PathBuf::from(&workspace.root_path).join(".agents/skills/draft-review");
            std::fs::create_dir_all(&skill).unwrap();
            std::fs::write(skill.join("SKILL.md"),"---\nname: Draft review\ndescription: Draft project skill\n---\nRead the mentioned file.").unwrap();
            let file = temp.path().join("context.txt");
            std::fs::write(&file, "Native draft attachment 中文").unwrap();
            let missing = temp.path().join("late.txt");
            let image = std::fs::read(
                repo.join("crates/agentkit-ui/examples/fixtures/attachment-preview.png"),
            )
            .unwrap();
            std::fs::write(temp.path().join("clipboard.png"), &image).unwrap();
            let paste =
                crate::files::from_bytes("clipboard:fixture".into(), "paste.png".into(), image)
                    .unwrap();
            let draft = g::SessionDraft {
                text: "[skill:.agents:draft-review] 😀 [file:context.txt] 待发送".into(),
                attachments: Some(vec![
                    g::SessionDraftAttachmentsItem {
                        path: file.to_string_lossy().into(),
                        name: "context.txt".into(),
                        content: None,
                    },
                    agentkit_store::core::drafts::to_ref(&paste).unwrap(),
                    g::SessionDraftAttachmentsItem {
                        path: missing.to_string_lossy().into(),
                        name: "late.txt".into(),
                        content: None,
                    },
                ]),
            };
            api.set_draft(&session.id, &draft).await.unwrap();
            assert_eq!(
                api.draft(&session.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .attachments
                    .unwrap()
                    .len(),
                3
            );
            crate::test_persistence::durable_bundle(sidecar.endpoint(), &workspace.id, &session.id)
                .await
                .unwrap();
            api.close();
            sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
            for stage in ["edit", "restore"] {
                let result = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "controller::draft_actions::tests::contract::draft_contract_child",
                        "--exact",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("AGENTKIT_CONFIG_DIR", &config)
                    .env("AGENTKIT_APP_DATA_DIR", &frontend)
                    .env("AGENTKIT_DRAFT_STAGE", stage)
                    .env("AGENTKIT_DRAFT_SESSION", &session.id)
                    .env("AGENTKIT_DRAFT_ROOT", temp.path())
                    .env_remove("AGENTKIT_REMOTE_URL")
                    .env_remove("AGENTKIT_REMOTE_TOKEN")
                    .status()
                    .unwrap();
                assert!(result.success(), "draft stage {stage}");
            }
            let data: serde_json::Value =
                serde_json::from_slice(&std::fs::read(config.join("drafts.json")).unwrap())
                    .unwrap();
            assert!(data["drafts"].get(&session.id).is_none());
            assert!(
                data["drafts"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|d| d["text"].as_str().is_some_and(|t| t.ends_with(" final")))
            );
        });
    }
    #[gpui_kit::test]
    #[ignore = "Owned by draft_contract_parent"]
    fn draft_contract_child(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let runtime: RuntimeSlot = Rc::new(RefCell::new(None));
        let startup: StartupSlot = Rc::new(RefCell::new(None));
        let quitting = Rc::new(Cell::new(false));
        let stage = std::env::var("AGENTKIT_DRAFT_STAGE").unwrap();
        let first = std::env::var("AGENTKIT_DRAFT_SESSION").unwrap();
        let root = PathBuf::from(std::env::var_os("AGENTKIT_DRAFT_ROOT").unwrap());
        let path =
            PathBuf::from(std::env::var_os("AGENTKIT_APP_DATA_DIR").unwrap()).join("prefs.json");
        let defaults = Preferences {
            onboarding_deferred: true,
            browser_initialized: true,
            session: Some(first.clone()),
            ..Default::default()
        };
        let prefs = PreferenceStore::new(
            path.clone(),
            crate::preferences::load(&path, defaults).unwrap(),
        );
        let (w, controller) = cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(crate::exit::ExitState::default());
            let capabilities = agentkit_capabilities::client_caps::NativeCapabilities::install(cx);
            let brand = Brand {
                app_name: "Draft contract".into(),
                bundle_id: "dev.shipagent.draft-test".into(),
                data_dir_name: ".shipagent-draft-test".into(),
                url_scheme: "shipagent-test".into(),
                default_theme: "light".into(),
            };
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    WorkbenchController::new(
                        brand,
                        runtime.clone(),
                        startup.clone(),
                        quitting.clone(),
                        WindowServices {
                            preferences: prefs.clone(),
                            capabilities,
                        },
                        window,
                        cx,
                    )
                })
            })
            .unwrap()
        });
        until(cx, "initial restore", |cx| {
            cx.update(|cx| {
                let c = controller.read(cx);
                if let agentkit_ui::recovery::Phase::Fatal { message, .. } =
                    c.view.read(cx).connection_state()
                {
                    panic!("{message}");
                }
                c.view.read(cx).connection_ready()
                    && c.state
                        .as_ref()
                        .is_some_and(|s| !s.read(cx).draft_sync.loading)
                    && c.draft_sync.lanes.values().any(|l| l.loaded)
            })
        });
        if stage == "edit" {
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    let store = c.state.clone().unwrap();
                    let s = store.read(cx);
                    assert_eq!(s.model.state.session.id, first);
                    assert_eq!(
                        s.model.draft.text,
                        "[skill:.agents:draft-review] 😀 [file:context.txt] 待发送"
                    );
                    assert_eq!(s.attachments.files.len(), 2);
                    assert_eq!(s.draft_sync.unavailable.len(), 1);
                    c.handle(WorkbenchIntent::Submit("blocked".into()), window, cx);
                    assert!(!store.read(cx).model.has_pending_send());
                    let skills = c.workspace_store.read(cx);
                    assert!(
                        skills.skills.iter().any(|s| s.slug == "draft-review"
                            && s.source == g::LoadedSkillSource::Project)
                    );
                })
            })
            .unwrap();
            let epoch = cx.update(|cx| controller.read(cx).transport_epoch);
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| c.restart_backend(false, window, cx))
            })
            .unwrap();
            until(cx, "backend restart", |cx| {
                cx.update(|cx| {
                    let c = controller.read(cx);
                    c.transport_epoch > epoch
                        && c.view.read(cx).connection_ready()
                        && c.state.as_ref().is_some_and(|s| {
                            !s.read(cx).draft_sync.loading
                                && s.read(cx).draft_sync.unavailable.len() == 1
                        })
                })
            });
            let api = cx.update(|cx| controller.read(cx).api().unwrap());
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.view.update(cx, |v, cx| {
                        v.apply_draft(
                            &first,
                            "[skill:.agents:draft-review] 😀 [file:context.txt] 待发送 R",
                            window,
                            cx,
                        )
                    });
                    c.prepare_draft_exit(cx);
                })
            })
            .unwrap();
            let persisted_writer = cx.update(|cx| controller.read(cx).draft_writer.clone());
            smol::block_on(persisted_writer.flush()).unwrap();
            assert_eq!(
                smol::block_on(api.draft(&first))
                    .unwrap()
                    .unwrap()
                    .attachments
                    .unwrap()
                    .len(),
                3
            );
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.view.update(cx, |v, cx| {
                        v.apply_draft(
                            &first,
                            "[skill:.agents:draft-review] 😀 [file:context.txt] 待发送 R",
                            window,
                            cx,
                        )
                    });
                    let store = c.state.clone().unwrap();
                    std::fs::write(
                        &store.read(cx).draft_sync.unavailable[0].reference.path,
                        "late file restored",
                    )
                    .unwrap();
                    c.handle(WorkbenchIntent::RetryDraft(first.clone()), window, cx);
                })
            })
            .unwrap();
            until(cx, "missing reference retry", |cx| {
                cx.update(|cx| {
                    controller.read(cx).state.as_ref().is_some_and(|s| {
                        s.read(cx).draft_sync.unavailable.is_empty()
                            && !s.read(cx).draft_sync.loading
                    })
                })
            });
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    let store = c.state.clone().unwrap();
                    let ids = store
                        .read(cx)
                        .attachments
                        .files
                        .iter()
                        .filter(|e| e.file.name == "late.txt" || e.file.name == "paste.png")
                        .map(|e| e.id.clone())
                        .collect::<Vec<_>>();
                    for id in ids {
                        c.handle(
                            WorkbenchIntent::RemoveAttachment {
                                session_id: first.clone(),
                                id,
                            },
                            window,
                            cx,
                        );
                    }
                });
                window.render_frame(cx);
                window.click("composer", cx);
                window.input(" +edited", cx);
                controller.update(cx, |c, cx| c.prepare_draft_exit(cx))
            })
            .unwrap();
            let writer = cx.update(|cx| controller.read(cx).draft_writer.clone());
            smol::block_on(writer.flush()).unwrap();
            assert!(
                smol::block_on(api.draft(&first))
                    .unwrap()
                    .unwrap()
                    .text
                    .ends_with("+edited")
            );
            let workspace =
                cx.update(|cx| controller.read(cx).workspace.as_ref().unwrap().id.clone());
            let second = smol::block_on(api.create_session_generated(&workspace, None)).unwrap();
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.handle(WorkbenchIntent::OpenSession(second.id.clone()), window, cx)
                })
            })
            .unwrap();
            until(cx, "open second session", |cx| {
                cx.update(|cx| {
                    let c = controller.read(cx);
                    c.state.as_ref().is_some_and(|s| {
                        s.read(cx).model.state.session.id == second.id
                            && !s.read(cx).draft_sync.loading
                    }) && c.draft_sync.lanes.get(&second.id).is_some_and(|l| l.loaded)
                })
            });
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.view.update(cx, |v, cx| {
                        v.apply_draft(&second.id, "后台草稿", window, cx)
                    })
                })
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.handle(WorkbenchIntent::OpenSession(first.clone()), window, cx)
                })
            })
            .unwrap();
            until(cx, "return first session", |cx| {
                cx.update(|cx| {
                    controller
                        .read(cx)
                        .state
                        .as_ref()
                        .is_some_and(|s| s.read(cx).model.state.session.id == first)
                })
            });
            let submitted = cx.update(|cx| {
                controller
                    .read(cx)
                    .state
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .model
                    .draft
                    .text
                    .clone()
            });
            let newfile = root.join("newer.txt");
            std::fs::write(&newfile, "newer attachment").unwrap();
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.handle(WorkbenchIntent::Submit(submitted.clone()), window, cx);
                    c.view
                        .update(cx, |v, cx| v.apply_draft(&first, "newer edit", window, cx));
                    c.handle(
                        WorkbenchIntent::Attach {
                            session_id: first.clone(),
                            inputs: vec![agentkit_ui::AttachmentInput::Path(newfile.clone())],
                        },
                        window,
                        cx,
                    );
                })
            })
            .unwrap();
            until(cx, "preserve newer edit after ACK", |cx| {
                cx.update(|cx| {
                    controller.read(cx).state.as_ref().is_some_and(|s| {
                        let s = s.read(cx);
                        if !s.attachments.sending
                            && s.attachments.reading == 0
                            && (!s.attachments.warnings.is_empty() || s.model.last_error.is_some())
                        {
                            panic!(
                                "draft upload failed: {:?} {:?}",
                                s.attachments.warnings, s.model.last_error
                            );
                        }
                        !s.attachments.sending
                            && s.attachments.reading == 0
                            && s.attachments.files.len() == 1
                    })
                })
            });
            cx.update(|cx| controller.update(cx, |c, cx| c.prepare_draft_exit(cx)));
            smol::block_on(writer.flush()).unwrap();
            let saved = smol::block_on(api.draft(&first)).unwrap().unwrap();
            assert_eq!(saved.text, "newer edit");
            assert_eq!(saved.attachments.unwrap()[0].name, "newer.txt");
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.handle(WorkbenchIntent::Submit("newer edit".into()), window, cx)
                })
            })
            .unwrap();
            until(cx, "clear consumed draft", |cx| {
                cx.update(|cx| {
                    controller.read(cx).state.as_ref().is_some_and(|s| {
                        !s.read(cx).attachments.sending && s.read(cx).model.draft.text.is_empty()
                    })
                })
            });
            cx.run_until_parked();
            smol::block_on(writer.flush()).unwrap();
            assert!(smol::block_on(api.draft(&first)).unwrap().is_none());
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.handle(WorkbenchIntent::OpenSession(second.id.clone()), window, cx)
                })
            })
            .unwrap();
            until(cx, "return second session", |cx| {
                cx.update(|cx| {
                    controller
                        .read(cx)
                        .state
                        .as_ref()
                        .is_some_and(|s| s.read(cx).model.state.session.id == second.id)
                })
            });
            let bytes = std::fs::read(root.join("clipboard.png")).unwrap();
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    c.handle(
                        WorkbenchIntent::Attach {
                            session_id: second.id.clone(),
                            inputs: vec![agentkit_ui::AttachmentInput::Image {
                                name: "second.png".into(),
                                bytes: bytes.into(),
                            }],
                        },
                        window,
                        cx,
                    )
                })
            })
            .unwrap();
            until(cx, "clipboard import", |cx| {
                cx.update(|cx| {
                    controller.read(cx).state.as_ref().is_some_and(|s| {
                        s.read(cx).attachments.reading == 0
                            && s.read(cx).attachments.files.len() == 1
                    })
                })
            });
        } else {
            cx.update_window(w, |_, window, cx| {
                controller.update(cx, |c, cx| {
                    let store = c.state.clone().unwrap();
                    assert_ne!(store.read(cx).model.state.session.id, first);
                    assert_eq!(store.read(cx).model.draft.text, "后台草稿");
                    assert_eq!(store.read(cx).attachments.files.len(), 1);
                    assert_eq!(store.read(cx).attachments.files[0].file.name, "second.png");
                    use base64::Engine as _;
                    assert_eq!(
                        base64::engine::general_purpose::STANDARD
                            .decode(
                                store.read(cx).attachments.files[0]
                                    .file
                                    .base64
                                    .as_ref()
                                    .unwrap()
                            )
                            .unwrap(),
                        std::fs::read(root.join("clipboard.png")).unwrap()
                    );
                });
                window.render_frame(cx);
                window.click("composer", cx);
                window.input(" final", cx);
            })
            .unwrap();
        }
        // Same synchronous editor capture and drain as ordinary Quit, even within the debounce window.
        cx.update(crate::controller::prepare_draft_exit);
        let writer = cx.update(|cx| controller.read(cx).draft_writer.clone());
        smol::block_on(writer.flush()).unwrap();
        smol::block_on(prefs.finish()).unwrap();
        let running = runtime.borrow_mut().take().unwrap();
        assert!(smol::block_on(running.shutdown()));
    }
}
