# T-1-12 native conversation views

Base main7ccfa57 after PR18. Implement spec03 ChatView/TurnCard/MarkdownView and spec04 conversation rows, phase indicators and F15/F19. Keep full ToolCard/Diff/Editor/child-agent nesting for T-1-13, composer/attachments and plan workflow in assigned later tasks. Stage0/performance/signing/release remain open.

1. Extract the history from WorkbenchView into an independent SessionStore-backed ChatView Entity; stable per-turn entities and per-message MarkdownView Entities. Preserve incremental delta/final correction and virtual-list splice/scroll anchoring. Bound retained rendering entities around the viewport without dropping canonical content.
2. Compose native user bubbles, assistant activity/response areas, derived phase indicators, completed activity summaries and copy/details actions. Render typed system errors with title/body/details and explicit retry/settings/link intents; route native capabilities and user-approved retry in the controller only.
3. Native regressions for phases/folding, Markdown identity/finalization/code/table/link/selection, system errors, scroll/cache/session boundaries; actual Linux bright/dark streaming screenshots, full three-platform checks and separate spec/standards review before commit/push/ready PR. No merge. NextT-1-13.
