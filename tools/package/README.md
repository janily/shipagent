# macOS nightly（T-1-22）

复用 PR #6 固定提交 d936cef 的打包/JIT/挂载验证代码，适配合并 PR #28 后的完整工作台；没有合并旧分支，也没有改动上游后端。需要原生 macOS、Rust 1.99.1、Node 22、Bun 1.3.14、uv、Xcode Command Line Tools。

```sh
cargo xtask package --target macos-arm64 --sign ad-hoc --probe
```

Intel 原生宿主使用 macos-x64。版本来自 workspace Cargo.toml，品牌来自 apps/workbench/brand.toml。产物在 target/packages/<target>：DMG、.app、SHA256SUMS、package-report.json；已有输出会被保留并拒绝覆盖。所有 Mach-O 逐个签名并核对 hardened-runtime、实际 entitlements 和签名后 hash，外层 app 最后签名。仅 Bun 使用两个 JIT entitlement；none/jit/jit-unsigned 探针记录实际 DFG 编译与 Wasm 结果，不以零退出码推断 JIT。

应用从 executable 所在 .app/Contents/Resources/backend 解析后端，资源缺失时不回落源码目录。显式 AGENTKIT_BACKEND_DIR 仍可用于开发。打包拒绝 WebUI、逃逸/循环符号链接并记录完整原生文件清单；应用和后端许可证与 NOTICE 随包提供，build-info.json 记录版本、源提交、后端固定版本及 probe 状态。

## 流水线与工程包

macos-nightly.yml 每晚、相关 main 推送、打版本 tag、手动和相关 PR 触发。当前 macos-15/arm64 与 macos-15-intel/x64 分别原生构建，不能将一个架构的结果冒充另一个。

默认和 PR 只运行 engineering：ad-hoc + hardened runtime，明确标记 engineering/probeBuild，不读取签名 Secrets、不提交 Apple。挂载 DMG 后移动源码后端并清除 BACKEND_DIR，在独立配置/CWD 和本地 fake-llm 下启动包内完整控制器，验证真实 Read、流式回答与完成帧，再运行包内真实后端持久化/取消契约，最终恢复源码后端并卸载。fixture 偏好显式跳过引导，避免首启向导挡住工程探针。失败不上传临时 profile、令牌或正文。

产物以 Actions artifact 保存14天，按架构、模式和提交命名；不自动创建 Release、不升级 beta/stable。只上传 DMG、白名单报告与校验和；不直接上传 .app，避免 ZIP 提取丢失 Unix 执行权限。220MB 体积结果列在报告，实体桌面、性能和干净机验收仍开放。

## 显式签名、公证路径

手动选择 developer-id，或 main 定时运行且仓库变量 MACOS_SIGNED_NIGHTLY=true，才进入签名模式；版本 tag 必须与 Cargo.toml 相同并强制签名。签名任务独立禁止 PR，且提交必须属于 main。缺少任何凭据或任一步失败都会失败，没有 ad-hoc 降级。

签名 job 绑定 macos-signing environment。以后由维护者将签名 Secrets 存于该环境，并将部署来源限制为 main/版本 tag；不要把它们作为任意同仓 PR 可读的仓库级 Secrets。工程包 job 不绑定该环境。可配置以下 Secret 名称（不要在聊天、仓库或 artifacts 中提供值）：

- APPLE_CERTIFICATE_P12_BASE64：含 Developer ID 私钥的 P12，base64 编码。
- APPLE_CERTIFICATE_PASSWORD：P12 导出密码。
- APPLE_SIGNING_IDENTITY：完整 Developer ID Application 标识，包含10位 Team ID。
- APPLE_NOTARY_KEY_ID、APPLE_NOTARY_ISSUER_ID、APPLE_NOTARY_PRIVATE_KEY_BASE64：App Store Connect 公证 API key 的标识与 PKCS#8 私钥。

setup 在 RUNNER_TEMP 建立权限受限的临时 keychain，保留原搜索列表，导入证书/公证 profile 后删除原始 P12/P8。签名/公证/验证后或失败时都恢复搜索列表并删除临时 keychain。命令失败不回显参数、stdout/stderr 或凭据。设置 helper 只允许在隔离 Actions runner 中使用，不改本地开发者 keychain。

```sh
# 本地已有合适身份及 notarytool profile 时，也可显式调用：
APPLE_SIGNING_IDENTITY='Developer ID Application: … (TEAMID1234)' \
APPLE_NOTARY_KEYCHAIN_PROFILE=existing-profile \
cargo xtask package --target macos-arm64 --sign developer-id --notarize
```

正式模式构建不带 perf 的普通 release；对 app 和 DMG 分别公证，只有 Accepted 才 staple/validate，随后执行 CI Gatekeeper assessment。普通签名包挂载验证只声称签名清单和真实后端契约，报告 nativeApplicationRound=not_evaluated；不把工程探针的应用回合当作普通签名包的实测。

当前没有 Developer ID Secrets。真实签名、公证、Gatekeeper、干净机及 Bun 的最小 Developer ID 权限仍未验收；按用户决定先保留工程包，不再索取凭据，不标记阶段0或发布就绪。

本地测试：node --test tools/package/*.test.mjs；cargo test -p xtask -p agentkit-sidecar。实际 codesign、hdiutil、两架构 JIT、挂载回合证据必须来自 macOS 流水线，Linux mock 不构成签名成功。
