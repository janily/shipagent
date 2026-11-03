//! Deterministic, referenced-only translation conversion and source gate.
mod catalog;
mod source;
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, fs, path::Path};

pub fn run(check: bool) -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace")?;
    generate(root, check)
}
fn generate(root: &Path, check: bool) -> Result<()> {
    let mut audit =
        source::Audit::with_exceptions(&fs::read_to_string(root.join("tools/i18n/non-ui.json"))?)?;
    for directory in [
        "crates/agentkit-app/src",
        "crates/agentkit-ui/src",
        "crates/agentkit-ui/examples",
        "apps",
    ] {
        scan(&root.join(directory), &mut audit)?;
    }
    audit.finish()?;
    let owned = root.join("crates/agentkit-app/locales");
    let upstream = root.join("backend/mkagent/packages/shared/src/i18n/locales");
    let en = catalog::load(&owned.join("en.json"))?;
    let zh = catalog::load(&owned.join("zh-CN.json"))?;
    catalog::parity(&en, &zh)?;
    for key in en.keys() {
        if !key.starts_with("ak.") {
            bail!("Owned key {key} must use the ak. prefix");
        }
        if !audit.used.contains_key(key) {
            bail!("Unused owned key {key}");
        }
    }
    let upstream_en = catalog::load(&upstream.join("en.json"))?;
    let upstream_zh = catalog::load(&upstream.join("zh-Hans.json"))?;
    let mut outputs = Vec::new();
    for (locale, own, up) in [("en", &en, &upstream_en), ("zh-CN", &zh, &upstream_zh)] {
        let mut values = BTreeMap::new();
        for (key, usages) in &audit.used {
            let value = if key.starts_with("ak.") {
                own.get(key)
            } else {
                up.get(key)
            }
            .with_context(|| format!("Missing {locale} key {key}"))?;
            let converted = catalog::convert(value, !key.starts_with("ak."))?;
            let parameters = catalog::parameters(&converted)?;
            for usage in usages {
                if parameters != usage.parameters {
                    bail!(
                        "{}: {key} expects {parameters:?}, received {:?}",
                        usage.location,
                        usage.parameters
                    );
                }
            }
            values.insert(key.clone(), converted);
        }
        outputs.push((
            root.join(format!("crates/agentkit-ui/locales/{locale}.json")),
            serde_json::to_string_pretty(&values)? + "\n",
        ));
    }
    let left: BTreeMap<String, String> = serde_json::from_str(&outputs[0].1)?;
    let right: BTreeMap<String, String> = serde_json::from_str(&outputs[1].1)?;
    catalog::parity(&left, &right)?;
    publish(&outputs, check)?;
    println!(
        "i18n {}: {} referenced keys, en/zh-CN, source and interpolation checks passed",
        if check { "verified" } else { "updated" },
        audit.used.len()
    );
    Ok(())
}
fn scan(path: &Path, audit: &mut source::Audit) -> Result<()> {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)?
            .map(|e| e.map(|v| v.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for entry in entries {
            scan(&entry, audit)?;
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        audit.file(path, &fs::read_to_string(path)?)?;
    }
    Ok(())
}
fn publish(outputs: &[(std::path::PathBuf, String)], check: bool) -> Result<()> {
    // Validate all inputs before writing; check mode must never mutate artifacts.
    let changed: Vec<_> = outputs
        .iter()
        .filter(|(path, text)| fs::read_to_string(path).ok().as_ref() != Some(text))
        .collect();
    if check && !changed.is_empty() {
        bail!(
            "Generated locale drift: {:?}; run cargo xtask i18n",
            changed.iter().map(|(p, _)| p).collect::<Vec<_>>()
        );
    }
    for (path, text) in changed {
        fs::create_dir_all(path.parent().context("locale parent")?)?;
        fs::write(path, text)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn check_mode_never_modifies_missing_or_changed_artifacts() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("locale.json");
        let outputs = vec![(path.clone(), "expected\n".into())];
        assert!(publish(&outputs, true).is_err());
        assert!(!path.exists());
        fs::write(&path, "edited")?;
        assert!(publish(&outputs, true).is_err());
        assert_eq!(fs::read_to_string(&path)?, "edited");
        publish(&outputs, false)?;
        publish(&outputs, true)?;
        Ok(())
    }
    fn fixture() -> Result<tempfile::TempDir> {
        let dir = tempfile::tempdir()?;
        for folder in [
            "crates/agentkit-ui/src",
            "crates/agentkit-app/src",
            "crates/agentkit-ui/examples",
            "apps",
            "tools/i18n",
            "crates/agentkit-app/locales",
            "backend/mkagent/packages/shared/src/i18n/locales",
        ] {
            fs::create_dir_all(dir.path().join(folder))?;
        }
        fs::write(dir.path().join("tools/i18n/non-ui.json"), "[]")?;
        fs::write(
            dir.path().join("crates/agentkit-ui/src/view.rs"),
            r#"fn view(){t!("ak.hello", name = user);t!("session.unreadGroup", count = n);}"#,
        )?;
        for (locale, text) in [("en", "Hello %{name}"), ("zh-CN", "你好 %{name}")] {
            fs::write(
                dir.path()
                    .join(format!("crates/agentkit-app/locales/{locale}.json")),
                serde_json::to_string(&serde_json::json!({"ak.hello":text}))?,
            )?;
        }
        for (locale, text) in [
            ("en", "Unread ({{count}})"),
            ("zh-Hans", "未读（{{count}}）"),
        ] {
            fs::write(
                dir.path().join(format!(
                    "backend/mkagent/packages/shared/src/i18n/locales/{locale}.json"
                )),
                serde_json::to_string(
                    &serde_json::json!({"session.unreadGroup":text,"unused.upstream":"Not selected"}),
                )?,
            )?;
        }
        Ok(dir)
    }
    #[test]
    fn pipeline_selects_converts_and_detects_drift_without_writing() -> Result<()> {
        let root = fixture()?;
        generate(root.path(), false)?;
        let output = root.path().join("crates/agentkit-ui/locales/en.json");
        let text = fs::read_to_string(&output)?;
        assert!(text.contains("%{count}"));
        assert!(!text.contains("unused.upstream"));
        generate(root.path(), true)?;
        fs::write(&output, "manual edit")?;
        assert!(generate(root.path(), true).is_err());
        assert_eq!(fs::read_to_string(&output)?, "manual edit");
        Ok(())
    }
    #[test]
    fn pipeline_rejects_missing_unused_or_wrong_parameters_before_publishing() -> Result<()> {
        for source in [
            r#"fn view(){t!("missing.key");}"#,
            r#"fn view(){t!("ak.hello", wrong = n);}"#,
            "fn view(){}",
        ] {
            let root = fixture()?;
            fs::write(root.path().join("crates/agentkit-ui/src/view.rs"), source)?;
            assert!(generate(root.path(), false).is_err());
            assert!(
                !root
                    .path()
                    .join("crates/agentkit-ui/locales/en.json")
                    .exists()
            );
        }
        Ok(())
    }
}
