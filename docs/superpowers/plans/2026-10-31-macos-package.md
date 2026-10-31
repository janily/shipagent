# macOS packaging trial implementation plan

> Execute inline with superpowers:executing-plans. User authorized continued development and commits/pushes.

**Goal:** T-0-09 reviewable app/DMG tooling, explicit hardened ad-hoc signing, Bun entitlement trials and a packaged native Agent round on macOS CI.
**Architecture:** Rust xtask orchestrates pinned backend/release builds and supplies centralized branding/version to dependency-free Node packaging. Bundle lookup is based on current executable. An explicit perf-enabled engineering build reuses the existing isolated native probe; default package builds omit it.
**Tech Stack:** Rust 1.99.1, Node22, Bun1.3.14, native macOS codesign/ditto/hdiutil/notarytool.
**Spec:** docs/spec/05 工程规范与质量.md packaging section; 06 T-0-09; 03 application startup.

## Constraints
- Backend remains pinned, patches outside submodule; no WebUI.
- Bundle backend at Contents/Resources/backend; no checkout-path fallback inside an app.
- Sign every discovered Mach-O individually, Bun baseline allow-jit + allow-unsigned-executable-memory; sign app last. Explicit ad-hoc versus Developer ID, never automatic fallback.
- No existing package output overwritten; no external/dangling symlinks bundled.
- Notarization requires explicit flag and preconfigured credentials, never attempted by PR CI. Do not claim Developer ID/Gatekeeper/notary/clean-machine acceptance from ad-hoc CI.
- Report actual DMG bytes/hash, app size and entitlement trials; no credentials/transcripts uploaded.

## Review focus
- Installed/mounted app must not depend on source-tree backend or current directory.
- Signing failures must not publish a partially signed app or overwrite a valid artifact.
- Extensionless Mach-O/node addons must be included in signing inventory; JIT permissions only on Bun.
- Unsafe brands, escaping symlinks and malformed CLI options fail before building/writing output.
- DMG verification must use the mounted payload and no backend environment override; failed native rounds never pass from successful process exit alone.

### Task1: Runtime bundle discovery
- [ ] Test packaged path with spaces and missing backend; development exe is not falsely classified.
- [ ] Implement helper in sidecar, app uses it after explicit backend override; core/native checks.
- [ ] Commit.

### Task2: Bundle/sign/DMG commands
- [ ] Node tests first: layout/plist, all Mach-O inventory, unsafe brands/external symlinks/missing artifacts/existing output refusal.
- [ ] Implement stage/sign pipeline, explicit sign modes, Bun JIT trials, DMG creation/checksum, optional Developer ID notary submit/staple; xtask CLI validates arguments/branding/version and orchestrates builds.
- [ ] Run Node/core suites; commit/push draft PR to start macOS release packaging CI.

### Task3: Mounted package validation
- [ ] Verify DMG readonly mount, codesign each native file/app, launch mounted probe build from isolated CWD with source backend moved aside and no BACKEND_DIR override, actual fake model/Read/final frame evidence; detach mount even on failure.
- [ ] Run normal packaged backend real RPC/Read/cancel tests against mounted resources.
- [ ] Record actual entitlement trial results/DMG size/limits, CI results; independent whole-branch review and fixes.
- [ ] Mark PR ready when relevant checks pass. T-0-09 stays partial until actual Developer ID/notary and clean-machine evidence.
