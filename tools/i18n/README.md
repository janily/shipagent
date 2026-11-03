# Native translation pipeline

Run `cargo xtask i18n` after adding or changing copy, and `cargo xtask i18n --check` to verify without writing files. No Node/Bun installation or backend build is needed; initialize the pinned `backend/mkagent` submodule.

Application code uses `agentkit_ui::t!("literal.key", name = value)` (or imports `t`). The macro routes through the UI crate's rust-i18n backend so app/UI share the active locale and fallback. `set_locale` normalizes Chinese variants to `zh-CN`; other unsupported languages use English. Named arguments are substituted by rust-i18n without recursively substituting inserted values. The gate requires exactly the parameters in both translations.

- Upstream input: `backend/mkagent/packages/shared/src/i18n/locales/{en,zh-Hans}.json`. Keep original upstream key names. Only referenced keys are emitted; unused upstream resources remain untouched.
- Native-owned input: `crates/agentkit-app/locales/{en,zh-CN}.json`, flat string values with `ak.` keys. All owned keys must be used by production or sample source, with identical bilingual keys and parameters.
- Generated output: `crates/agentkit-ui/locales/{en,zh-CN}.json`. Commit these artifacts. Check mode catches missing files, manual edits and stale keys without modifying them.

The converter handles i18next `{{name}}` / `{{ name }}` as rust-i18n `%{name}`. Repeated named parameters work. Formatting, unescaped HTML interpolation and nested `$t()` references fail explicitly; adapt these before use. Explicit plural keys (`_one`, `_other`) can be selected by Rust branches; this pipeline does not implement i18next plural selection.

The source gate parses Rust syntax throughout app/UI sources, UI examples and application entry points, regardless of platform or feature flags. It collects literal `t!()` usages including nested macros, checks named arguments, detects Chinese and English sentences, and rejects direct single-word copy in label/title/placeholder/accessibility/content methods. String conversions and scoped local initializer aliases retain visible-text context. Tests, comments and developer diagnostics are not product copy; translations nested in diagnostic expressions are still collected. External user/model/backend data stays unchanged. This is a source policy checker, not a Rust type/dataflow analyzer for arbitrary external functions.

`non-ui.json` records exact file/literal exceptions for identifiers, file/URL data and fixed benchmark input, each with a reason. Duplicate, unexplained or unused entries fail. Review additions as code; do not exempt user-facing prose.

CI's Linux `i18n` job runs pipeline regression tests and the read-only check. Native runtime localization is also exercised by all three `native-check` jobs. Run `cargo test -p xtask i18n` for focused tool tests.
