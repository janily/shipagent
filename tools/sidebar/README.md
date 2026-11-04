# Native navigation verification

The ignored `navigation` integration test runs against `AGENTKIT_BACKEND_DIR` and verifies real create/rename/delete, workspace isolation, refusal to remove the active workspace, and data/identity preservation after unregister/reopen. CI runs it on Linux/macOS/Windows.

```
cargo test -p agentkit-sidecar --test navigation --locked -- --ignored
cargo test -p agentkit-ui --features samples --test sidebar --test permissions --test workbench --locked
```

For an isolated Linux X11/Vulkan display with xdotool and Python/Pillow:

```
cargo build -p workbench --locked
cargo build -p agentkit-sidecar --example navigation-fixture --locked
node tools/sidebar/smoke.mjs target/debug/workbench target/backend/linux-x64 target/debug/examples/navigation-fixture NEW.json light
```

Use `dark` for the second theme. The seed example creates 1,000 sessions through the ordinary typed API in a new temporary profile, plus an empty second workspace. The runner uses native clicks/keys to rename, cancel deletion, retain a draft, switch workspace, create another session and switch back to the remembered session; it captures real windows and checks normal exit/process cleanup. No app-side navigation bypass exists. Reports are created exclusively and never overwritten. The example refuses an existing data profile and accepts only a loopback model endpoint.

This verifies rendering/operations on the available software-rendered desktop. It does not accept physical desktop, reference-machine scroll performance, signing, notarization or release gates. Native headless component tests verify that 1,000 rows are virtualized and selection/keyboard/search follow canonical metadata; no 60 fps claim is inferred from them.
