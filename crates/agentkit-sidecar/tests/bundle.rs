use agentkit_sidecar::bundled_backend_directory;
use std::path::{Path, PathBuf};
#[test]
fn app_backend_is_relative_to_executable_even_when_missing() {
    assert_eq!(
        bundled_backend_directory(Path::new(
            "/Volumes/Trial Space/Workbench.app/Contents/MacOS/workbench"
        )),
        Some(PathBuf::from(
            "/Volumes/Trial Space/Workbench.app/Contents/Resources/backend"
        ))
    );
}
#[test]
fn development_or_unrelated_macos_folder_is_not_a_bundle() {
    assert_eq!(
        bundled_backend_directory(Path::new("/checkout/target/release/workbench")),
        None
    );
    assert_eq!(
        bundled_backend_directory(Path::new("/checkout/Contents/MacOS/workbench")),
        None
    );
    assert_eq!(
        bundled_backend_directory(Path::new("/Wrong.app/not-contents/MacOS/workbench")),
        None
    );
}
