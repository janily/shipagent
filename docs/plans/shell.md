# T-1-10 native application shell

Base f814cd7 after merged PR16. Implement spec03 application startup/preferences/single-instance and spec04 fixed three-column shell, F03. No stage0/performance/release acceptance.

1. Test native-independent policy seams: validated/versioned preferences (layout, theme/language, geometry, last session), atomic background latest-value writes; interprocess local socket/named pipe single-instance with OS-held lock, bounded focus protocol and stale-socket handling. No foreground filesystem IO.
2. Restore preferences/theme/language/window before backend, show startup skeleton, health/bootstrap and last session fallback. Existing no-model endpoint setup remains; full onboarding/settings later. Central shell actions, resizable sidebar/right panel, narrow Sheet, focus mode, honest context/plan/task shell contents and StatusBar.
3. Native interaction, keyboard/resize/persistence, real two-launch focus smoke, theme screenshots and three-platform checks. Independent spec/standards review, commit/push/ready PR; no merge. Deeper files/plan/task UI and full menu/keymap customization follow assigned tasks.
