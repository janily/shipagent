use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use syn::{
    Expr, Lit, Token,
    parse::Parser,
    visit::{self, Visit},
};
#[derive(Default)]
pub struct Audit {
    pub used: BTreeMap<String, Vec<Usage>>,
    errors: Vec<String>,
    path: String,
    visible: bool,
    diagnostic: bool,
    bindings: BTreeMap<String, Expr>,
    resolving: BTreeSet<String>,
    exceptions: Vec<Exception>,
}
#[derive(serde::Deserialize)]
struct Exception {
    file: String,
    literal: String,
    reason: String,
    #[serde(skip)]
    used: bool,
}
pub struct Usage {
    pub location: String,
    pub parameters: BTreeSet<String>,
}
impl Audit {
    pub fn with_exceptions(text: &str) -> Result<Self> {
        let exceptions: Vec<Exception> = serde_json::from_str(text)?;
        let mut unique = BTreeSet::new();
        for item in &exceptions {
            if item.reason.trim().is_empty() || !unique.insert((&item.file, &item.literal)) {
                bail!("Non-UI exceptions need unique file/literal pairs and a reason");
            }
        }
        Ok(Self {
            exceptions,
            ..Self::default()
        })
    }
    pub fn file(&mut self, path: &Path, text: &str) -> Result<()> {
        self.path = path.display().to_string();
        self.bindings.clear();
        self.resolving.clear();
        self.visit_file(&syn::parse_file(text).with_context(|| self.path.clone())?);
        Ok(())
    }
    pub fn finish(&self) -> Result<()> {
        if !self.errors.is_empty() {
            bail!("i18n source violations:\n{}", self.errors.join("\n"));
        }
        for item in &self.exceptions {
            if !item.used {
                bail!("Unused non-UI exception {}: {:?}", item.file, item.literal);
            }
        }
        Ok(())
    }
    fn location(&self, span: proc_macro2::Span) -> String {
        format!("{}:{}", self.path, span.start().line)
    }
    fn error(&mut self, span: proc_macro2::Span, message: &str) {
        self.errors
            .push(format!("{}: {message}", self.location(span)));
    }
    // Diagnostic/json token grammars differ from Rust expression lists. Preserve
    // nested translation usages without interpreting their data strings as UI.
    fn nested_translations(&mut self, tokens: proc_macro2::TokenStream) {
        use proc_macro2::TokenTree;
        let tokens: Vec<_> = tokens.into_iter().collect();
        let mut index = 0;
        while index < tokens.len() {
            if let TokenTree::Ident(name) = &tokens[index]
                && name == "t"
                && matches!(tokens.get(index+1), Some(TokenTree::Punct(p)) if p.as_char()=='!')
                && let Some(TokenTree::Group(group)) = tokens.get(index + 2)
            {
                let body = group.stream();
                let node: syn::Macro = syn::parse_quote!(t!(#body));
                self.translation(&node);
                index += 3;
                continue;
            }
            if let TokenTree::Group(group) = &tokens[index] {
                self.nested_translations(group.stream());
            }
            index += 1;
        }
    }
    fn translation(&mut self, node: &syn::Macro) {
        let Ok(args) = syn::punctuated::Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(node.tokens.clone())
        else {
            self.error(
                node.path.segments[0].ident.span(),
                "t! requires a literal key and named arguments",
            );
            return;
        };
        let Some(Expr::Lit(key)) = args.first() else {
            self.error(
                node.path.segments[0].ident.span(),
                "Dynamic translation key; use t!(\"literal.key\")",
            );
            return;
        };
        let Lit::Str(key) = &key.lit else {
            self.error(
                node.path.segments[0].ident.span(),
                "Translation key must be a string",
            );
            return;
        };
        let mut parameters = BTreeSet::new();
        for arg in args.iter().skip(1) {
            if let Expr::Assign(assign) = arg
                && let Expr::Path(path) = assign.left.as_ref()
                && let Some(name) = path.path.get_ident()
            {
                if !parameters.insert(name.to_string()) {
                    self.error(name.span(), "Duplicate translation parameter");
                }
                self.visit_expr(&assign.right);
            } else {
                self.error(
                    key.span(),
                    "Translation parameters must use name = expression",
                );
            }
        }
        let location = self.location(key.span());
        self.used.entry(key.value()).or_default().push(Usage {
            location,
            parameters,
        });
    }
}
fn test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || a.path().is_ident("cfg")
                && a.meta
                    .require_list()
                    .is_ok_and(|m| m.tokens.to_string() == "test")
    })
}
fn han(value: &str) -> bool {
    value.chars().any(|c| matches!(c, '\u{3400}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{2ffff}'))
}
impl<'ast> Visit<'ast> for Audit {
    fn visit_block(&mut self, node: &'ast syn::Block) {
        let outer = self.bindings.clone();
        visit::visit_block(self, node);
        self.bindings = outer;
    }
    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit::visit_local(self, node);
        if let syn::Pat::Ident(name) = &node.pat
            && let Some(init) = &node.init
        {
            self.bindings
                .insert(name.ident.to_string(), (*init.expr).clone());
        }
    }
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if self.visible
            && let Some(name) = node.path.get_ident()
        {
            let name = name.to_string();
            if let Some(value) = self.bindings.get(&name).cloned()
                && self.resolving.insert(name.clone())
            {
                self.visit_expr(&value);
                self.resolving.remove(&name);
            }
        }
    }
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {} // docs, cfg and diagnostic attributes are not UI text.
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !test_only(&node.attrs) {
            visit::visit_item_mod(self, node);
        }
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if !test_only(&node.attrs) {
            visit::visit_item_fn(self, node);
        }
    }
    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        if node.ident.is_none() {
            self.visit_macro(&node.mac);
        }
    }
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        let name = node
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        if name == "t" {
            self.translation(node);
            return;
        }
        // Developer diagnostics and compile-time resources are not product copy.
        if matches!(
            name.as_str(),
            "i18n"
                | "include_str"
                | "include_bytes"
                | "env"
                | "option_env"
                | "eprintln"
                | "println"
                | "bail"
                | "anyhow"
                | "ensure"
                | "json"
                | "assert"
                | "assert_eq"
                | "assert_ne"
                | "debug_assert"
                | "trace"
                | "debug"
                | "info"
                | "warn"
                | "error"
        ) {
            self.nested_translations(node.tokens.clone());
            return;
        }
        if name == "matches" {
            let parser = |input: syn::parse::ParseStream<'_>| {
                let value = input.parse::<Expr>()?;
                let _ = input.parse::<Token![,]>()?;
                let _ = input.parse::<proc_macro2::TokenStream>()?;
                Ok(value)
            };
            if let Ok(value) = parser.parse2(node.tokens.clone()) {
                self.visit_expr(&value);
            } else {
                self.error(
                    node.path.segments[0].ident.span(),
                    "Invalid matches! syntax",
                );
            }
            return;
        }
        // Parse nested expressions inside format!, vec!, etc.; do not scan token
        // text with regex (comments, raw strings and nested macros need Rust syntax).
        if let Ok(args) = syn::punctuated::Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(node.tokens.clone())
        {
            for (index, arg) in args.iter().enumerate() {
                if name == "format"
                    && index == 0
                    && let Expr::Lit(lit) = arg
                    && let Lit::Str(text) = &lit.lit
                {
                    let mut plain = String::new();
                    let mut chars = text
                        .value()
                        .chars()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .peekable();
                    while let Some(ch) = chars.next() {
                        if ch == '{' && chars.peek() == Some(&'{') {
                            chars.next();
                            plain.push('{');
                        } else if ch == '}' && chars.peek() == Some(&'}') {
                            chars.next();
                            plain.push('}');
                        } else if ch == '{' {
                            let mut field = String::new();
                            for ch in chars.by_ref() {
                                if ch == '}' {
                                    break;
                                }
                                field.push(ch);
                            }
                            let name = field.split(':').next().unwrap_or_default().trim();
                            if let Ok(path) = syn::parse_str::<syn::ExprPath>(name) {
                                self.visit_expr_path(&path);
                            }
                        } else {
                            plain.push(ch);
                        }
                    }
                    self.visit_lit_str(&syn::LitStr::new(&plain, text.span()));
                } else {
                    self.visit_expr(arg);
                }
            }
        } else {
            self.error(
                node.path.segments[0].ident.span(),
                "Unrecognized macro body in UI source; add an explicit audited syntax handler",
            );
        }
    }
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        // Err is a developer diagnostic. UI-facing errors must be mapped at the
        // controller boundary, separately from server/user data.
        if let Expr::Path(p) = node.func.as_ref()
            && p.path.is_ident("Err")
        {
            let old = self.diagnostic;
            self.diagnostic = true;
            for arg in &node.args {
                self.visit_expr(arg);
            }
            self.diagnostic = old;
            return;
        }
        if let Expr::Path(p) = node.func.as_ref() {
            let function = p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            if p.path.segments.iter().any(|s| s.ident == "Alert")
                && matches!(
                    function.as_str(),
                    "new" | "error" | "warning" | "info" | "success"
                )
            {
                let old = self.visible;
                for (index, arg) in node.args.iter().enumerate() {
                    self.visible = index > 0;
                    self.visit_expr(arg);
                }
                self.visible = old;
                return;
            }
            if matches!(function.as_str(), "tr" | "translate") {
                self.error(
                    p.path.segments[0].ident.span(),
                    "Use literal t! calls, not runtime translation functions",
                );
            }
            if function == "new" || matches!(function.as_str(), "h_resizable" | "v_resizable") {
                let old = self.visible;
                let identifiers = matches!(function.as_str(), "h_resizable" | "v_resizable")
                    || p.path.segments.iter().any(|s| {
                        matches!(
                            s.ident.to_string().as_str(),
                            "Button" | "Sidebar" | "Settings" | "MessageScroller"
                        )
                    });
                let labels = p.path.segments.iter().any(|s| {
                    matches!(
                        s.ident.to_string().as_str(),
                        "SettingPage" | "SettingItem" | "SidebarGroup" | "SidebarMenuItem"
                    )
                });
                self.visible = labels || old && !identifiers;
                for arg in &node.args {
                    self.visit_expr(arg);
                }
                self.visible = old;
                return;
            }
        }
        visit::visit_expr_call(self, node);
    }
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        self.visit_expr(&node.receiver);
        if matches!(
            name.as_str(),
            "context" | "with_context" | "expect" | "ok_or" | "ok_or_else"
        ) {
            let old = self.diagnostic;
            self.diagnostic = true;
            for arg in &node.args {
                self.visit_expr(arg);
            }
            self.diagnostic = old;
            return;
        }
        let old = self.visible;
        self.visible = matches!(
            name.as_str(),
            "child"
                | "label"
                | "title"
                | "description"
                | "placeholder"
                | "aria_label"
                | "accessibility_label"
                | "set_window_title"
                | "with_jump_button_label"
                | "show_error"
        );
        for arg in &node.args {
            self.visit_expr(arg);
        }
        self.visible = old;
    }
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        let old = self.visible;
        self.visible = false;
        self.visit_expr(&node.cond);
        self.visible = old;
        self.visit_block(&node.then_branch);
        if let Some((_, branch)) = &node.else_branch {
            self.visit_expr(branch);
        }
    }
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        let old = self.visible;
        self.visible = false;
        self.visit_expr(&node.expr);
        self.visible = old;
        for arm in &node.arms {
            self.visit_expr(&arm.body);
        }
    }
    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        if self.diagnostic {
            return;
        }
        let value = node.value();
        if let Some(item) = self
            .exceptions
            .iter_mut()
            .find(|item| Path::new(&self.path).ends_with(&item.file) && value == item.literal)
        {
            item.used = true;
            return;
        }
        let sentence = value
            .split_whitespace()
            .filter(|w| w.chars().any(|c| c.is_ascii_alphabetic()))
            .count()
            >= 2;
        if han(&value) || sentence || self.visible && value.chars().any(|c| c.is_alphabetic()) {
            self.error(
                node.span(),
                &format!("Hardcoded UI copy {value:?}; use t!(\"key\")"),
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scans_qualified_nested_and_raw_literal_keys_but_not_comments_or_tests() -> Result<()> {
        let mut audit = Audit::default();
        audit.file(
            Path::new("ui.rs"),
            r##"
            // t!("unused.comment")
            fn view() { div().child(format!("{}", agentkit_ui::t!(r#"upstream.key"#, count = n))); }
            #[cfg(test)] mod tests { fn fake(){ div().child("测试文本"); t!("unused.test"); } }
        "##,
        )?;
        audit.finish()?;
        assert_eq!(audit.used.keys().collect::<Vec<_>>(), vec!["upstream.key"]);
        assert!(audit.used["upstream.key"][0].parameters.contains("count"));
        Ok(())
    }
    #[test]
    fn rejects_dynamic_calls_and_copy_including_single_word_labels_and_hidden_locals() -> Result<()>
    {
        for body in [
            r#"t!(key)"#,
            r#"div().label("Send")"#,
            r#"Alert::error("alert-id", "Failure")"#,
            r#"div().label("Send".to_string())"#,
            r#"div().label(format!("{{Send}}"))"#,
            r#"div().child(format!("{{中文}}"))"#,
            r#"let label = "Send"; div().label(format!("{label}"))"#,
            r#"div().title("Settings".into())"#,
            r#"let label = "Send"; div().label(label)"#,
            r#"div().child(format!("Hello {name}"))"#,
            r#"let text = "硬编码""#,
            r#"let text = "Hello there""#,
            r#"t!("key", n = 1, n = 2)"#,
        ] {
            let mut audit = Audit::default();
            audit.file(Path::new("ui.rs"), &format!("fn view() {{ {body}; }}"))?;
            assert!(audit.finish().is_err(), "{body}");
        }
        Ok(())
    }
    #[test]
    fn diagnostic_expressions_still_collect_nested_translation_keys() -> Result<()> {
        let mut audit = Audit::default();
        audit.file(
            Path::new("app.rs"),
            r#"fn boot() {
            anyhow::ensure!(ready, "{}", agentkit_ui::t!("upstream.one"));
            let _=Err(t!("upstream.two"));
            check().context(t!("upstream.three"));
            serde_json::json!({"label": t!("upstream.four")});
        }"#,
        )?;
        audit.finish()?;
        assert_eq!(audit.used.len(), 4);
        Ok(())
    }
    #[test]
    fn non_ui_exceptions_are_exact_explained_and_cannot_become_stale() -> Result<()> {
        let entry = r#"[{"file":"view.rs","literal":"README.md","reason":"file name data"}]"#;
        let mut audit = Audit::with_exceptions(entry)?;
        audit.file(
            Path::new("view.rs"),
            r#"fn view(){div().child("README.md");}"#,
        )?;
        audit.finish()?;
        let mut stale = Audit::with_exceptions(entry)?;
        stale.file(Path::new("view.rs"), "fn view(){}")?;
        assert!(stale.finish().is_err());
        let mut other_file = Audit::with_exceptions(entry)?;
        other_file.file(
            Path::new("other.rs"),
            r#"fn view(){div().child("README.md");}"#,
        )?;
        assert!(other_file.finish().is_err());
        assert!(
            Audit::with_exceptions(r#"[{"file":"view.rs","literal":"text","reason":""}]"#).is_err()
        );
        Ok(())
    }
}
