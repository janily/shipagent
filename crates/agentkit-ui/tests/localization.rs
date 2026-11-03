//! Exercise the generated catalogs through the same runtime as native controls.
use agentkit_ui::{set_locale, t};
#[test]
fn generated_catalogs_translate_upstream_and_owned_named_parameters() {
    set_locale("zh-Hans");
    assert_eq!(t!("common.failed"), "失败");
    assert_eq!(t!("ak.task_count", count = 3), "后台任务 3");
    set_locale("en");
    assert_eq!(t!("common.failed"), "Failed");
    assert_eq!(t!("ak.task_count", count = 3), "Background tasks 3");
    set_locale("unsupported");
    assert_eq!(t!("common.cancel"), "Cancel");
}
