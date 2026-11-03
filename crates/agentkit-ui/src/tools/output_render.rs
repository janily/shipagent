use super::*;
use gpui_kit::component::{diff::Diff, skeleton::Skeleton};
impl Render for ToolOutputView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex()
            .id(SharedString::from(format!(
                "tool-output:{}",
                self.preview.id
            )))
            .w_full()
            .gap_2()
            .p_3();
        body = body.child(div().text_sm().font_semibold().child(t!("ak.tools.input")));
        for (key, field) in &self.preview.fields {
            let key = key.clone();
            let caption = key.clone();
            body = body.child(div().text_xs().font_semibold().child(caption));
            body = body.child(
                div()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_sm()
                    .child(field.text.clone()),
            );
            if field.truncated {
                body = body.child(
                    Button::new(SharedString::from(format!(
                        "input-full:{}:{key}",
                        self.preview.id
                    )))
                    .ghost()
                    .label(t!("ak.tools.open_field"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(ToolContent::Field(key.clone()), cx);
                    })),
                );
            }
        }
        if self.preview.input_cut {
            body = body.child(div().text_xs().child(t!("ak.tools.input_limited")));
        }
        body = body.child(
            Button::new("tool-input-full")
                .ghost()
                .label(t!("ak.tools.open_input"))
                .on_click(cx.listener(|this, _, _, cx| this.request(ToolContent::Input, cx))),
        );
        let editor_height = rems(
            self.preview
                .result
                .text
                .lines()
                .count()
                .saturating_add(1)
                .clamp(2, 10) as f32
                * 1.5
                + 1.0,
        );
        let mut result = v_flex()
            .id("tool-result")
            .gap_2()
            .w_full()
            .when(self.preview.failed, |v| {
                v.border_l_2().border_color(cx.theme().danger).pl_3()
            });
        let changes = matches!(
            self.preview.source,
            Source::Patch { .. } | Source::Change { .. }
        );
        result = result.child(div().text_sm().font_semibold().child(if changes {
            t!("ak.tools.changes")
        } else {
            t!("ak.tools.result")
        }));
        if self.loading {
            result = result.child(Skeleton::new());
        } else if let Some(diff) = &self.diff {
            result = result.child(
                div()
                    .id("tool-diff")
                    .h_64()
                    .w_full()
                    .child(Diff::new(diff))
                    .test_support(),
            );
        } else if let Some(editor) = &self.editor {
            if matches!(
                self.preview.source,
                Source::Patch { .. } | Source::Change { .. }
            ) {
                result = result.child(div().text_xs().child(t!("ak.tools.diff_unavailable")));
            }
            result = result.child(
                Editor::new(editor)
                    .readonly(true)
                    .h(editor_height)
                    .aria_label(t!("ak.tools.result")),
            );
        } else {
            if matches!(
                self.preview.source,
                Source::Patch { .. } | Source::Change { .. }
            ) {
                result = result.child(div().text_xs().child(t!("ak.tools.diff_unavailable")));
            }
            result = result.child(
                div()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_sm()
                    .child(self.preview.result.text.clone()),
            );
        }
        if self.diff.is_some() && !self.preview.result.text.is_empty() {
            result = result.child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child(t!("ak.tools.execution_result")),
            );
            if let Some(editor) = &self.editor {
                result = result.child(
                    Editor::new(editor)
                        .readonly(true)
                        .h(editor_height)
                        .aria_label(t!("ak.tools.execution_result")),
                );
            }
        }
        if self.preview.result.truncated {
            result = result.child(div().text_xs().child(t!("ak.tools.truncated")));
        }
        result = result.child(
            Button::new("tool-result-full")
                .ghost()
                .label(t!("ak.tools.open_result"))
                .on_click(cx.listener(|this, _, _, cx| this.request(ToolContent::Result, cx))),
        );
        body.child(result.test_support()).test_support()
    }
}
