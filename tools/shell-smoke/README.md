# Native application shell smoke

The main three-platform native-check job runs the real OS socket/named-pipe ownership and separate-process focus tests, preference persistence/failure/quit policies and native UI controls. Linux/macOS/Windows all compile the complete application and open a native Kit sample frame; this does not claim a physical desktop or full app-frame acceptance on macOS/Windows.

For the actual Linux workbench, build `cargo build -p workbench --features perf`, start an isolated X11 display, and run:

```
node tools/shell-smoke/run.mjs target/debug/workbench target/backend/linux-x64 NEW.json light
node tools/shell-smoke/run.mjs target/debug/workbench target/backend/linux-x64 OTHER.json dark
```

The runner owns a temporary backend config, a separate client preference directory and a local scripted model. It opens the actual workbench, launches the binary a second time with the same profile (must forward focus and exit), sends Ctrl+Shift+B to the native focused window, checks persisted right-panel state, completes the actual Read/streamed reply, checks appearance/geometry/last-session persistence and verifies the existing Linux subreaper's owned-process cleanup report. Sensitive environment keys are removed. Existing report files are never overwritten. `AGENTKIT_SHELL_READY_FILE` optionally writes the X11 window identity after Read completes for a separate screenshot capture; the app adds no screenshot-specific product flow.

Reports intentionally say performance/physical-desktop acceptance `not_evaluated`. The optional perf feature supplies existing instrumentation and the isolated Agent driver; debug values are functional evidence only. The ordinary performance runner now explicitly isolates its client directory too, keeping the backend-empty-directory invariant intact.
