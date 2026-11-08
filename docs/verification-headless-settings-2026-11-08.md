# T-2-01 headless 代理与防休眠偏好

基线 PR #31 合并提交 `85fc3a4`；后端固定 v0.2.6 `6e8ebc4`，子模块不变。见[本轮计划](plans/headless-settings.md)。

## 行为与边界

P-01 补齐 standalone 的代理保存 handler，并在网络 bootstrap 前应用已保存配置。保存返回 `{ restartRequired: true }`，不会热改当前进程；Rust 设置 API 与 Workbench facade 提供类型化回执。HTTP/HTTPS 独立路由、HTTPS 回退、NO_PROXY 的 host/suffix/端口/IPv6/通配符规则与 Electron 一致。直接引用真正的 undici 实现，同时处理 Bun 原生 fetch 与预加载拦截器；无配置/禁用时清理继承代理，防止环境回退。

P-02 严格校验布尔值，仅保存 keep-awake 偏好。设置写入前检查配置、写后读回确认；损坏/缺失配置、无效 URL/转义凭据不能虚假 ACK，固定错误文本不携带秘密。真实平台防休眠归 T-2-19，设置 UI 归 T-2-02。完整上游检查发现 P-05 原有测试使用动态 import 值作为类型，已改用 InstanceType；运行逻辑不变。

## 验证

新增 Bun 测试 9 项通过，包括本地两代理/HTTP origin 的真实 Bun 与 undici 请求、HTTPS CONNECT 选择与拒绝、fallback、端口匹配/不匹配、IPv6 loopback、继承坏代理时无配置/禁用直连、参数与配置损坏、保存不热应用。每种配置独立进程，测试不依赖公网代理或禁用 TLS 校验。完整补丁构建共 23 项 Bun 回归。

真实 sidecar 契约先在旧后端复现 ChannelUnavailable，再在新构建验证：直接连接→保存 A→旧进程保持直连→新 token/连接代际重启后走 A→保存 B 仍走 A→重启走 B→NO_PROXY 重启后直连→清空配置且保留继承代理再重启仍直连。keep-awake true/false 跨重启读回。测试只使用私有配置、虚构凭据和自有本地 HTTP 服务。

P-01/P-02 独立应用与全部 P-01–P-05 均通过完整上游 `bun run validate:ci`：TypeScript、shared 测试、20 项文档工具 smoke、1206 键 i18n parity/usage/sort。`xtask` 新增 opt-in 完整上游门禁，Linux contract CI 强制启用；三平台 CI 新增真实 headless_settings 契约。本地已实际通过启用此门禁的完整 `xtask backend build --contract`，验证和打包均成功。首次基线文档 smoke 因只读 HOME 的 uv cache 失败，改用私有 UV_CACHE_DIR 后 20 项通过，没有改写 HOME。

核心 Rust 280 项、类型化 API 5 项、Node 63 项通过；fmt、全 workspace 全特性全目标 clippy、35 根协议与 330 键原生国际化防漂移、变更 workflow actionlint 通过。两项独立审查覆盖规格与规范，修复了缺失配置虚假 ACK、继承代理回退和重启代际等待。

[Linux 原始原生回归报告](e2e/linux-t201.json)四次启动 full/restore/defer/defer_restore 与 8 项隔离拒绝检查通过。使用已有正式 e2e 构建与本轮新后端；本轮无 GUI 修改，当前 Rust 修改另经类型检查和核心测试。三平台及工程包最终状态以本轮 PR 最新 head 检查为准。

## 仍待完成

[P-01/P-02 上游贡献文案](upstream/T-2-01-pr.md)已准备，尚未发布，等待用户授权。第三方 PR 发布不影响本仓库补丁审阅。HTTPS fixture 故意拒绝 CONNECT，只证明路由选择；公网 TLS、企业 CA 与真实订阅不在本轮验收内。阶段0/P0、实体桌面 IME/辅助技术、参考机性能与 Developer ID/公证门槛保持开放。

下一项 T-2-02：原生网络代理、通知与电源、权限、发送键及高级设置。
