# macOS package trial (T-0-09)

Native macOS only; Rust1.99.1, Node22+, Bun1.3.14+, uv and Xcode Command Line Tools. Version comes from workspace Cargo.toml, brand from apps/workbench/brand.toml. Backend commit/patch discipline is unchanged.

```sh
cargo xtask package --target macos-arm64 --sign ad-hoc
```

Native Intel hosts use macos-x64. This creates target/packages/<target>/Workbench.app, DMG, SHA256SUMS and package-report.json. Existing output is refused before building: preserve/move an earlier package before rebuilding. No forced replacement. The default build is release without the probe feature. macOS deployment floor is13.0; current CI host is macOS14 arm64, not proof for Intel or macOS13.

The pipeline builds the pinned backend and release executable, stages a private app with Contents/Resources/backend, signs each Mach-O (including extensionless binaries, .node/.dylib, Bun, rg and uv), signs the outer app last and verifies signatures. Non-Bun code gets empty entitlements. Bun baseline uses allow-jit and allow-unsigned-executable-memory. Reports audit the resulting entitlement keys, CodeDirectory runtime flag and final signed binary hashes, not source binary hashes. No WebUI or escaping/dangling symlinks are accepted. Failed staging/signing/DMG creation does not publish partial output or alter an existing artifact.

Bun trials separately sign copies with no entitlements, allow-jit only, and both baseline keys. The pinned bun:jsc probe requires observed DFG compilation plus a Wasm result; a zero exit or successful interpreter alone cannot pass. Trials do not automatically reduce the package's baseline permissions, nor establish the minimal Developer ID policy for all tools/addons.

## Developer ID and notarization

```sh
cargo xtask package --target macos-arm64 --sign developer-id --notarize
```

Requires APPLE_SIGNING_IDENTITY naming an already available `Developer ID Application:` identity and APPLE_NOTARY_KEYCHAIN_PROFILE naming an already configured notarytool profile. A release CI setup must import these temporarily from CI Secrets; this PR's ad-hoc trial workflow does not import credentials or submit anything to Apple. The command has no ad-hoc fallback. Certificates, private keys and passwords must not be committed or placed in artifacts.

Explicit notarization submits a ditto archive of the signed app, waits for Accepted, staples/verifies the app, builds/signs the DMG, submits the final DMG and staples/verifies it. This path is implemented but remains unverified until actual credentials are provided. Ad-hoc signing does not establish Gatekeeper, notarization or clean-machine acceptance.

## Engineering probe and mounted verification

```sh
cargo xtask package --target macos-arm64 --sign ad-hoc --probe
```

This explicitly marks an engineering DMG and enables the existing perf feature; ordinary launch still does not auto-run without the isolated marker/env protocol. Do not publish it as the default production build. The macOS package trial workflow verifies the mounted engineering payload using tools/package/verify-mounted.mjs:

1. Mount DMG readonly, verify every signed native file/hash and app.
2. Move the source-tree backend aside; clear BACKEND_DIR and credential/remote/probe env; use isolated config/fixture/CWD outside the checkout.
3. Launch the mounted executable. Its normal controller talks to the bundled backend and the local scripted model, executes real Read, streams final text, records a completed final frame and quits through awaited shutdown.
4. Run the independent real backend persistence/cancellation contract against mounted Resources/backend.
5. Restore the source backend and detach the DMG in finally; never infer success from process exit alone.

Automatic mounted verification is restricted to GitHub Actions; it is a controlled engineering probe, not a general local launcher. A failed/hung app is killed after65s; failure reports explicitly leave process-tree cleanup unverified, and the runner's tracked-process cleanup/disposal owns residual processes. The isolated directory is retained on failure rather than uploaded (no token/config/transcript artifacts). Successful runs delete it. Mounted-report failure never passes from otherwise fast frame metrics. Performance and clean-machine acceptance remain not_evaluated.

PR artifacts contain the DMG and JSON/checksums. Raw .app files are not uploaded directly because artifact ZIP extraction can lose Unix modes; install/test from the DMG. The installer-size comparison uses220,000,000 bytes (decimal220MB), with actual byte count and SHA256 recorded.

Tests: `node --test tools/package/*.test.mjs`; core Rust tests cover bundle discovery and invalid CLI inputs. Actual codesign/hdiutil/DFG/installed-path evidence must come from the macOS workflow, not Linux fixture tests.
