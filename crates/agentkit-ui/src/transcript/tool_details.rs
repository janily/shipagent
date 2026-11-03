use super::*;
impl TurnCard {
    pub(super) fn toggle_tool(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.tools.remove(id) {
            self.outputs.remove(id);
        } else {
            self.tools.insert(id.into());
            self.create_output(id, window, cx);
        }
        self.remeasure(cx);
        cx.notify();
    }
    pub(super) fn create_output(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let prepared = self
            .store
            .read(cx)
            .model
            .turns
            .get(self.row)
            .and_then(|turn| {
                if let Turn::Assistant(turn) = turn {
                    turn.activities.iter().find(|a| a.id == id)
                } else {
                    None
                }
            })
            .map(crate::tools::ToolOutputView::prepare);
        let Some(prepared) = prepared else {
            self.outputs.remove(id);
            return;
        };
        if self
            .outputs
            .get(id)
            .is_some_and(|(view, _)| view.read(cx).matches(&prepared))
        {
            return;
        }
        let view = cx.new(|cx| crate::tools::ToolOutputView::from_prepared(prepared, window, cx));
        let intent = cx.subscribe(&view, |_, _, event: &ChatIntent, cx| cx.emit(event.clone()));
        let layout = cx.subscribe(&view, |this, _, _: &crate::tools::ToolOutputEvent, cx| {
            this.remeasure(cx);
            cx.notify();
        });
        self.outputs.insert(id.into(), (view, vec![intent, layout]));
    }
}
