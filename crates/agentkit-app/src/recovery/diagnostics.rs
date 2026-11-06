use super::*;
impl WorkbenchController {
    pub(in crate::controller) fn recovery_logs(&mut self, cx: &mut Context<Self>) {
        let path = self.recovery_log_dir();
        self.operations.push(cx.spawn(async move |weak, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let path = path?;
                    std::fs::create_dir_all(&path).map_err(|_| ())?;
                    agentkit_capabilities::open_local_path(&path).map_err(|_| ())
                })
                .await;
            if result.is_err() {
                let _ = weak.update(cx, |this, cx| {
                    this.view.update(cx, |v, cx| {
                        v.show_error(agentkit_ui::t!("ak.recovery.logs_failed"), false, cx)
                    })
                });
            }
        }));
    }
    fn recovery_log_dir(&self) -> Result<PathBuf, ()> {
        let root = if let Some(path) = std::env::var_os("AGENTKIT_CONFIG_DIR") {
            PathBuf::from(path)
        } else {
            PathBuf::from(
                std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok_or(())?,
            )
            .join(&self.brand.data_dir_name)
        };
        Ok(backend_log_dir(&self.brand, &root))
    }
    pub(in crate::controller) fn export_recovery(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let info = self.api().map(|a| a.snapshot());
        let report = serde_json::json!({"schemaVersion":1,"appVersion":env!("CARGO_PKG_VERSION"),"backendVersion":info.as_ref().and_then(|s|s.server_version.clone()),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"state":self.view.read(cx).connection_state().category(),"recoveryId":info.map(|s|s.recovery_id),"containsLogs":false});
        let picker = cx.prompt_for_new_path(&std::env::temp_dir(), Some("shipagent-recovery.json"));
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    let path = picker.await.map_err(|_| ())?.map_err(|_| ())?;
                    let Some(path) = path else {
                        return Ok(());
                    };
                    cx.background_executor()
                        .spawn(async move {
                            let parent = path.parent().ok_or(())?;
                            let mut file =
                                tempfile::NamedTempFile::new_in(parent).map_err(|_| ())?;
                            serde_json::to_writer_pretty(&mut file, &report).map_err(|_| ())?;
                            file.as_file().sync_all().map_err(|_| ())?;
                            file.persist(path).map_err(|_| ())?;
                            Ok::<_, ()>(())
                        })
                        .await
                }
                .await;
                if result.is_err() {
                    let _ = weak.update(cx, |this, cx| {
                        this.view.update(cx, |v, cx| {
                            v.show_error(agentkit_ui::t!("ak.recovery.export_failed"), false, cx)
                        })
                    });
                }
            }));
    }
}
