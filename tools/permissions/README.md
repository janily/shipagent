# Approval integration checks

`cargo test -p agentkit-sidecar --test permissions --locked -- --ignored` uses `AGENTKIT_BACKEND_DIR`, Node and an isolated local scripted model. It checks real file writes after allow, no write after deny, continuation to completion, and a remembered grant avoiding another prompt on the next round. CI contract runs this on Linux/macOS/Windows.

For a real Linux workbench, build `cargo build -p workbench --features perf --locked`, start isolated X11/Vulkan with xdotool and Python/Pillow, and run:

```
node tools/permissions/smoke.mjs target/debug/workbench target/backend/linux-x64 NEW.json light allow
node tools/permissions/smoke.mjs target/debug/workbench target/backend/linux-x64 OTHER.json dark deny
```

The runner owns its temporary profile/backend config and local model. It changes Safe→Ask through the native Shift+Tab input action, waits for the real approval, captures the window, then presses Enter/Esc (or secondary-Enter with `always`). Successful write cases also change Ask→AllowAll and capture the execute warning. The application uses its ordinary permission controller; no test-only approval bypass is installed. Existing reports are never overwritten. Reports verify real backend file side effects, final submitted frame and owned-process cleanup, and explicitly do not accept physical desktop or reference-machine performance gates.

System approval alerts use generic private copy, a local `approval_notifications` preference and bounded replay deduplication. Linux/Windows delivery uses notify-rust; macOS opts into its UNUserNotificationCenter backend and requests OS authorization. OS service denial is recoverable. Actual physical toast presentation, macOS bundled authorization and full alert settings/click navigation remain explicit desktop/release and T-2-15 checks.
