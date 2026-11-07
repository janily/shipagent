# T-1-22 macOS nightly 流水线

基线：合并 PR #28 的 ba9adaf。复用未合并 PR #6 固定提交 d936cef 的工程包实现，逐项适配当前工作台，不合并旧分支。

- [x] 安装布局与 package 命令：版本/品牌来自 Cargo.toml/brand.toml，后端从应用内解析，缺失不得回落源码；原有输出不得覆盖，安全 staging，完整原生文件签名清单。
- [x] macOS arm64/x64 定时、手动与 PR 工程验证：ad-hoc/hardened runtime 工程 DMG、JIT 证据、挂载后真实 Agent 回合与后端契约、校验和/来源/签名状态报告。无凭据时只保留工程包。
- [x] 显式 Developer ID 模式：临时 keychain 导入与清理、公证 API profile、逐文件签名、app/DMG 公证与 staple、Gatekeeper CI 检查；签名失败绝不降级。PR 不接触签名 Secrets；tag 缺凭据失败。
- [x] 脚本/策略/失败路径测试、macOS 实际工程包流水线配置、三平台原回归（最终运行结果见 PR）与独立 Spec/Standards 审查；commit、推送和 PR，不合并。

正式签名/公证、干净机与 Bun 最小 Developer ID entitlement 验收仍需凭据和对应实测。用户已决定凭据尚未配置，先保留工程包，不再次询问。阶段0仍开放；下一项 T-1-23。
