# T-2-02 原生网络、通知、电源、权限与高级设置

基线：PR #32 合并提交 `2b3a187`，后端仍固定 v0.2.6。依据 06·T-2-02、01·F-34、04·设置页及 05·分层/异步/隐私要求推进。

- [x] 扩展现有 Kit Settings 页面；复用 Theme、Input、Button、AlertDialog 和双语资源。沿用现有视觉稿，不另建网页或改变主题。
- [x] 网络字段默认遮蔽；组内编辑不提交，离开字段组或 Enter 后校验并提示中断影响，确认后保存并自动重启拥有的 sidecar。取消保留草稿，失败可重试；远程连接禁止此本地重启操作。
- [x] 本地回合完成/审批通知开关；后端防休眠偏好即改即存。实际系统通知/防休眠仍归 T-2-15/T-2-19，页面明确当前范围。
- [x] 当前工作区默认权限模式与后端内置策略只读展示；执行模式须显式确认。发送键沿用通用页并验证持久化与 IME 行为。
- [x] 高级页复用现有打开日志、无秘密诊断导出和有运行任务时确认重启。崩溃上报阶段3，不添加假开关。
- [x] 状态/代际保护、界面与真实后端契约、亮暗截图、独立规格/规范审查。
- [ ] 三平台 CI、commit/push/PR（最终证据见本轮 PR）。

界面所有者：设置导航/滚动由 Kit Settings 管理，输入编辑及 IME 由 Kit InputState 管理，控制器独占 RPC/sidecar，PreferenceStore 独占本地持久化，恢复模块独占重启/诊断。加载、失败、断线禁用和重连读回沿用设置页状态；不得把未 ACK 值显示为已保存，不在错误或诊断中包含代理 URL/凭据。

## 本轮 Canonical UI Map

| Capability | Canonical owner | Source of truth | Allowed variants | Verification |
| --- | --- | --- | --- | --- |
| Form | Kit InputState/Input/Switch；SettingsView | 04·设置页、DESIGN.md、settings/proxy.rs | 代理遮蔽输入、即存偏好；共享主题 | extended_settings.rs、亮暗原生截图 |
| Scrollbar | Kit Settings | 04·设置页、settings/render.rs | 单一内容区滚动；最小 960×600 | 原生窄窗口截图 |
| Dialog | Kit AlertDialog；settings_system.rs | 01·F-34、04·网络、05·权限 | 代理中断确认、默认执行确认；取消保留值 | settings_contract_parent |
| Toast | SettingsView.status；controller | 05·异步与隐私、settings_actions.rs | busy/error/notice；固定脱敏错误 | extended_settings.rs、真实控制器失败契约 |

阶段0/P0、实体桌面输入法/辅助技术、参考机性能及正式签名/公证门槛继续开放。
