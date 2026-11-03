use anyhow::{Context, Result, bail};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::Path,
};
pub type Catalog = BTreeMap<String, String>;
struct Unique(Catalog);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Keys;
        impl<'de> Visitor<'de> for Keys {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("flat unique string translations")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut values = Catalog::new();
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    if values.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "Duplicate translation key {key}"
                        )));
                    }
                }
                Ok(Unique(values))
            }
        }
        deserializer.deserialize_map(Keys)
    }
}
pub fn load(path: &Path) -> Result<Catalog> {
    let Unique(values) = serde_json::from_str(
        &fs::read_to_string(path).with_context(|| format!("Read {}", path.display()))?,
    )
    .with_context(|| format!("Parse {}", path.display()))?;
    Ok(values)
}
pub fn parity(en: &Catalog, zh: &Catalog) -> Result<()> {
    if en.keys().collect::<Vec<_>>() != zh.keys().collect::<Vec<_>>() {
        bail!("Locale key sets differ");
    }
    for (key, value) in en {
        if parameters(value)? != parameters(&zh[key])? {
            bail!("Locale interpolation differs for {key}");
        }
    }
    Ok(())
}
fn name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
}
pub fn convert(value: &str, upstream: bool) -> Result<String> {
    let mut result = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("{{") {
        if !upstream {
            bail!("Owned translations must use %{{name}} interpolation");
        }
        result.push_str(&rest[..start]);
        let end = rest[start + 2..]
            .find("}}")
            .context("Unclosed i18next interpolation")?
            + start
            + 2;
        let variable = rest[start + 2..end].trim();
        if !name(variable) {
            bail!("Unsupported i18next interpolation {variable:?}; named variables only");
        }
        result.push_str(&format!("%{{{variable}}}"));
        rest = &rest[end + 2..];
    }
    result.push_str(rest);
    if result.contains("}}") || result.contains("$t(") {
        bail!("Unsupported i18next nesting/interpolation");
    }
    parameters(&result)?;
    Ok(result)
}
pub fn parameters(value: &str) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut rest = value;
    while let Some(start) = rest.find("%{") {
        let end = rest[start + 2..]
            .find('}')
            .context("Unclosed rust-i18n interpolation")?
            + start
            + 2;
        let variable = &rest[start + 2..end];
        if !name(variable) {
            bail!("Invalid interpolation name {variable:?}");
        }
        names.insert(variable.into());
        rest = &rest[end + 1..];
    }
    Ok(names)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_repeated_named_parameters_without_changing_unicode() -> Result<()> {
        assert_eq!(
            convert("你好 {{ name }} / {{name}}: {{count}}", true)?,
            "你好 %{name} / %{name}: %{count}"
        );
        for bad in [
            "{{- html}}",
            "{{count, number}}",
            "{{open",
            "$t(nested)",
            "{{}}",
        ] {
            assert!(convert(bad, true).is_err(), "{bad}");
        }
        assert!(convert("{{name}}", false).is_err());
        Ok(())
    }
    #[test]
    fn rejects_duplicate_nested_or_non_string_resources() {
        for value in [
            r#"{"a":"one","a":"two"}"#,
            r#"{"a":{"b":"nested"}}"#,
            r#"{"a":1}"#,
        ] {
            assert!(serde_json::from_str::<Unique>(value).is_err());
        }
    }
    #[test]
    fn detects_key_and_parameter_mismatch() {
        let en = Catalog::from([("a".into(), "Hi %{name}".into())]);
        assert!(parity(&en, &Catalog::new()).is_err());
        assert!(parity(&en, &Catalog::from([("a".into(), "你好 %{who}".into())])).is_err());
    }
}
