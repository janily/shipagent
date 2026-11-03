# T-1-13 native tool cards

Base3607cd7 after PR19. Implement spec03 tool rendering, spec04 summary/error/nesting and F16. Full background task polling/stop is T-2-11; custom renderer registration is T-4-02.

1. Add bounded presentation policy for Read/code, Edit/Write/Pi/Codex diff inputs and generic tools, honest truncation (2000 lines with a 256KiB guard), folded JSON fields, and parent grouping via the existing upstream-compatible projection. Preserve actual activity identities/status; cap visible indentation at4.
2. Replace basic rows with a reusable native ToolCard (summary only while collapsed), lazy Diff/read-only Editor/input/result views, error indicator and explicit full-result/input editor intent. Full contents come from canonical session state and remain read-only; no tool execution or filesystem writes from views.
3. Native tests for collapse/lazy creation, diff/edit/write/malformed fallback, code read-only behavior, 100k-line truncation/full content, nested/cycle/orphan groups, latest results and session boundaries. Linux real Read screenshots in both themes and three-platform CI; separate spec/standards review before commit/push/ready PR. Stage0/performance/physical/signing/release gates remain open. NextT-1-14.
