# T-1-22 macOS nightly 流水线验证

基线为合并 PR #28 的 ba9adaf。复用 PR #6 固定提交 d936cef 的打包、JIT、挂载探针模块，逐项适配当前完整工作台，不合并旧分支、不覆盖原工程包证据。下一项 T-1-23 性能基准与 CI perf。

## 实现及边界

增加 cargo xtask package、应用内后端路径解析、macos-nightly.yml。版本/品牌从 Cargo.toml 与 brand.toml 读取；tag 必须与 Cargo workspace 版本相同。安装位置缺少 backend 时明确失败，不回落源码路径。应用/后端许可证和 NOTICE、源提交/后端固定版本/探针标识随包保存；包内所有 Mach-O 逐一签名、验证 hardened runtime、实际 entitlements 与签名后 SHA256，外层 app 最后签名。

工程模式使用 ad-hoc，DMG 名和报告明确标记 engineering/probe，不称为正式签名或公证。只有 Bun 使用两项 JIT entitlement；三种策略探针要求实际 DFG 编译和 Wasm 结果。两个架构分别在 macos-15 与 macos-15-intel 原生构建；runner 标签依据官方 runner-images 支持表，不以跨架构静态检查代替实测。

PR、定时、相关 main 推送、手动及版本 tag 均有入口。无凭据默认生成工程包；developer-id 为手动显式选择或受控定时 opt-in，版本 tag 强制签名。签名任务独立禁止 PR、提交须在 main，并绑定 macos-signing 环境；未来凭据仅存该受保护环境。临时 keychain 保留/恢复原搜索列表，原始 P12/P8 在导入后删除，成功及失败均清理；缺凭据、签名或公证失败绝不降级。app 和 DMG 分别 Accepted 后 staple/validate，再做 CI Gatekeeper assessment。产物是 Actions artifacts，不创建 Release、不推广 beta/stable。

工程 DMG 挂载前校验 DMG hash，挂载后校验来源、原生文件清单与签名。移走源码后端、去除后端覆盖及凭据环境，使用独立配置/CWD 与已延期引导的 fixture 偏好，运行当前完整控制器的真实 Read、流式最终回答、完成帧及包内后端持久化/取消契约，最后恢复源码后端并卸载。普通签名 release 不带 perf；其挂载验证只声明签名/来源和后端契约，nativeApplicationRound 明确 not_evaluated，不挪用工程探针结果。

## 验证

本地 Rust 默认核心280项通过，Node 16项实际断言通过（除 --test 文件级汇总外，逐个直接执行 test module 复核），覆盖安装布局、源信息/NOTICE、非法选项、已有输出保留、越界/循环链接、Mach-O/JIT 证据、挂载回合判断、模式/ref/tag 门禁、缺凭据与失败清理、命令错误脱敏。fmt、核心 clippy、329个国际化引用键、原生应用检查及 actionlint 通过。两路独立 Spec/Standards 审查问题已修复。

最终提交的三平台 CI 与两架构 macOS 实际工程包结果见本 PR 流水线；每个工程产物包含 package-report.json、mounted-verification.json、SHA256SUMS 和 DMG，记录实际字节数、220MB 比较、原生签名清单与来源。不预填体积、Gatekeeper 或公证结果。运行说明与未来 Secrets 名称见 [打包说明](../tools/package/README.md)。

目前无 Developer ID 凭据，按用户决定继续保留工程包，不再次索取凭据。因此真实 Developer ID 签名、公证、Gatekeeper、干净机与最小 Bun entitlement 仍待对应实测，T-0-09 正式验收没有关闭。工程包与 macOS CI 不是实体桌面、macOS13、参考硬件性能或阶段0整体验收；阶段0和发布门槛仍开放。
