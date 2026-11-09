# T-2-02 原生扩展设置

基线为 PR #32 合并提交 `2b3a187`。后端与子模块仍固定 v0.2.6，未增加补丁或改协议频道。对应 06·T-2-02 与 01·F-34 的设置页面部分；[计划及控件所有权](plans/extended-settings.md)。

## 行为

原生 Kit Settings 增加权限、网络、通知与电源、高级分组，保留通用/外观/模型/快捷键。代理地址默认遮蔽，可通过 Kit 控件显示/隐藏。字段组内编辑不触发保存；离开组或 Enter 后校验，显式提示所有工作区运行回合会中断，确认后调用类型化保存 API，收到 restartRequired 才自动重启拥有的 sidecar。取消及保存失败保留输入，其他设置读回不会覆盖草稿。失败固定文案不含凭据；新连接代际丢弃旧结果并重新读取，远程连接禁用本地代理修改/重启。

通知开关保存到客户端偏好，兼容旧 prefs；既有审批通知立即读取新开关。keep-awake 只在后端 ACK/读回后更新选择状态。完整回合通知与点击导航归 T-2-15，实际 OS 防休眠归 T-2-19，页面明确当前集成范围。通用页发送键继续经本地偏好/后端频道应用，未改变 Composer 的 IME 防发送实现。

默认权限绑定当前工作区，仅影响新会话；默认执行模式另有可取消确认。后端内置策略以只读文本显示。高级页复用日志目录、现有诊断导出及运行中确认重启；诊断只含版本/平台/连接代际等固定字段，不含日志、会话正文、代理或凭据；错误在设置页可见，诊断异步结果不能解除进行中的设置保存状态。阶段3崩溃上报未添加假开关。

## 验证

- 新增 5 项 GPUI 测试及原有 9 项设置测试通过：本地/后端开关事件分离、遮蔽与错误、单次 proposal、组内编辑不提交、取消→其他设置读回→重试保留草稿、工作区绑定/忙时禁用、远程禁用及高级操作事件。
- 应用 40 项单测通过，5 项依赖真实资源的测试默认忽略；真实 settings_contract_parent 另显式执行通过：取消前后配置相同，执行确认后才改工作区默认值，损坏配置导致保存失败且不重启/不泄露，修复后重试成功、新 token/代际、草稿与 keep-awake 跨重启保留；通知与发送键从磁盘读回。新的契约步骤加入三平台 native-check CI。
- 核心 Rust 280 项基线通过，新增 lexer 回归后最终 281 项通过；新增 i18n lexer 回归另验证 Switch ID 不误判为文案，Switch label 仍须翻译。fmt、全 workspace 全特性全目标 clippy、35 根 codegen 与双语键集检查通过（最终结果以本轮 PR 最新 head 为准）。
- 两项独立审查分别检查规格与工程规范，修复了其他设置读回覆盖代理草稿、诊断失败回调过早解除 busy、被拒绝 proposal 未释放及弹窗缺少取消按钮。
- premium 静态审查在本轮明确的控件所有权地图上通过；其源码扫描器面向 Web，不解析 Rust，不能替代 GPUI/控制器测试。

新环境的单实例测试在无网络权限 sandbox 中报 EPERM；允许本地 socket 后全数通过。上游 ripgrep-prebuilt 下载返回 403/504，本地使用 crates.io 锁定源码构建 ripgrep 15.0.0（上游 v15.0.1 发布配置固定该版本；额外补丁作用于 Windows/aarch64 构建）；仅作本地开发工具缓存，非官方预编译产物。仓库锁、CI 和正式打包路径不变，三平台 CI 继续验证正式下载产物。

## 原生视觉记录

Linux Xvfb/lavapipe 中实际运行 `cargo run -p agentkit-ui --features samples --example settings`；使用明确的展示夹具，没有模型请求或保存凭据。截图用于验证同一 SettingsView 的视觉与布局，真实业务由上面的控制器契约验证。以下均为原生截图：

- [网络亮色](images/settings-network-light.png) / [网络暗色](images/settings-network-dark.png)
- [只读权限](images/settings-permissions-light.png) / [通知与电源暗色](images/settings-power-dark.png)
- [英文高级页 960×600](images/settings-advanced-narrow.png)

## 保留的门槛

最新三平台和工程包结果见 PR。截图/语义输入测试不替代物理 IME、VoiceOver/NVDA 或实体桌面验收。完整系统通知、防休眠能力、阶段0/P0、参考机性能、Developer ID/公证继续开放；不会把本任务页面完成标记为整个 F-34 或发布验收完成。

下一项 T-2-03：ChatGPT Plus OAuth。
