use super::*;
use agentkit_protocol::RpcValue;
use gpui_kit::{AsyncApp, PathPromptOptions, SharedString};
use std::path::PathBuf;
pub(super) fn result(paths: Option<Vec<PathBuf>>) -> Result<RpcValue, WireError> {
    let paths = paths.unwrap_or_default();
    let paths = paths
        .into_iter()
        .map(|p| {
            p.into_os_string()
                .into_string()
                .map_err(|_| failure("Selected path is not UTF-8"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::from_value(serde_json::json!({"canceled":paths.is_empty(),"filePaths":paths}))
        .map_err(|_| failure("Invalid file selection result"))
}
pub(super) async fn pick(
    spec: &FileSpec,
    window: Option<AnyWindowHandle>,
    cx: &mut AsyncApp,
    alive: &std::sync::atomic::AtomicBool,
) -> Result<RpcValue, WireError> {
    // The GPUI TestPlatform stands in for native panels in UI/transport tests.
    // macOS production needs rfd's explicitly parented sheet; GPUI opens a standalone panel.
    let platform_picker = !cfg!(target_os = "macos") || cx.update(|cx| cx.is_test());
    if platform_picker && window.is_some() && spec.default_path.is_none() && spec.filters.is_empty()
    {
        let receiver = cx.update(|cx| {
            if !alive.load(std::sync::atomic::Ordering::Acquire) {
                return Err(unavailable("Native request canceled"));
            }
            if let Some(window) = window {
                let _ = window.update(cx, |_, window, _| window.activate_window());
            }
            Ok(cx.prompt_for_paths(PathPromptOptions {
                files: spec.files(),
                directories: spec.directories(),
                multiple: spec.multiple(),
                prompt: spec.title.clone().map(SharedString::from),
            }))
        })?;
        let paths = receiver
            .await
            .map_err(|_| failure("File picker closed"))?
            .map_err(|_| failure("File picker unavailable"))?;
        return result(paths);
    }
    if spec.files() && spec.directories() {
        return Err(unavailable(
            "Combined file/directory selection with filters is unavailable",
        ));
    }
    // GPUI 0.8.1 lacks default path/filter fields. Use a parented native picker for these options.
    let default = spec.default_path.clone();
    let default = cx
        .background_executor()
        .spawn(async move {
            default.map(|p| {
                let directory = p.is_dir();
                (p, directory)
            })
        })
        .await;
    let dialog = cx.update(|cx| {
        if !alive.load(std::sync::atomic::Ordering::Acquire) {
            return Err(unavailable("Native request canceled"));
        }
        let mut dialog = rfd::AsyncFileDialog::new()
            .set_can_create_directories(spec.properties.iter().any(|p| p == "createDirectory"));
        if let Some(title) = &spec.title {
            dialog = dialog.set_title(title);
        }
        if let Some((path, directory)) = &default {
            if *directory {
                dialog = dialog.set_directory(path);
            } else {
                if let Some(parent) = path.parent() {
                    dialog = dialog.set_directory(parent);
                }
                if let Some(name) = path.file_name() {
                    dialog = dialog.set_file_name(name.to_string_lossy());
                }
            }
        }
        for filter in &spec.filters {
            dialog = dialog.add_filter(&filter.name, &filter.extensions);
        }
        if let Some(window) = window {
            dialog = window
                .update(cx, |_, window, _| {
                    window.activate_window();
                    dialog.set_parent(window)
                })
                .map_err(|_| failure("Parent window closed"))?;
        }
        Ok(dialog)
    })?;
    let paths = match (spec.directories(), spec.multiple()) {
        (true, true) => dialog.pick_folders().await,
        (true, false) => dialog.pick_folder().await.map(|p| vec![p]),
        (false, true) => dialog.pick_files().await,
        (false, false) => dialog.pick_file().await.map(|p| vec![p]),
    };
    result(paths.map(|v| v.into_iter().map(|p| p.path().to_path_buf()).collect()))
}
