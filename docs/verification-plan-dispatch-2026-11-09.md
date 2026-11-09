# T-2-05 PlanCard 与待执行计划调度器

基线为 PR #35 合并提交 `e8b0f48`；对应 01·U4/F-24、03·plan_dispatch/PlanCard、04·计划卡与 06·T-2-05，[计划](plans/plan-dispatch.md)。上游仍固定 v0.2.6，协议频道与生成根不变；本轮新增 P-06，原 P-01–P-05 不改。

## 原生流程

PlanCard 是独立 Entity，使用 Kit GroupBox、Markdown/TextView 与 Button。只有最后一条非隐藏计划提供动作，历史计划只读；忙、正在发送、设置切换、计划操作或文件未读成功时禁用。接受确认后显示“已接受”；等待压缩和等待执行确认分别提示。草稿非空时说明将作为补充上下文。

固定后端会把路径直接放在 plan message.content；优先显式 planPath，兼容 Unix/Windows 绝对路径，内联正文仍直接渲染。计划文件通过新增类型化 `file:read`（String、30 秒只读策略）由后端校验工作区路径；原生 UI 不直接读取本机文件。失败可重试，读取带连接代际，旧结果不能解除新读取；会话打开/恢复会补读仍未完成或失败的卡片。内容和页脚变化触发虚拟行重测，防止长 Markdown 被旧 loading 行高裁切。

接受时捕获当前草稿及其 revision，探索 safe 先收到 allow-all ACK，再发送当前语言的批准消息；ask 保持原选择。消息与上游 plan-approval-message 相同，以分隔线和额外上下文标题附加 trim 后草稿，不把路径写进批准文本。接受并压缩先持久保存 planPath/draftInputSnapshot，再发 `/compact`。原接受 ACK 与后续派发错误分别回报；只清理仍属于捕获版本的草稿，后续编辑保留。

## 持久派发与边界

会话打开/恢复、compaction_complete、忙→idle 和 complete 均只作唤醒。每个服务端分配的 clientId、连接代际及会话共享单飞并合并唤醒；旧 bound transport 的退场不压住新连接恢复。worker 查询真实会话及持久 pending，等待/忙/已认领均不派发，只有原子 claim 明确 true 才发送；认领后 detached worker 继续完成绑定会话的发送，不依赖当前窗口是否仍显示该会话。send ACK 接受后才 clear，任何失败都向界面报告，首错不能被后续 idle 掩盖。

保持固定协议的 at-most-once 语义：已认领但发送结果不确定的记录不自动重发；这不是 claim/send/clear 跨进程的 exactly-once 事务。短会话或失败压缩仍 awaiting，不根据文本、超时或客户端 markCompactionComplete 猜测就绪；用户可在会话中显式 `/compact` 重试。

## P-06

真实回归先成功手动压缩，再重启后端；磁盘 pending 已 ready，但 claim 被拒。根因是 headerToMetadata 刻意省略 pendingPlanExecution，而 ensureMessagesLoaded 只恢复 messages/tokenUsage，内存仍无记录；普通 metadata 写回也可能擦除它。

P-06 在既有消息懒加载处恢复字段，prepare/claim/clear 在改状态前确保加载。不会启动时全量读历史、不扩大 SessionMetadata 或 Session wire；认领标记仍在首次 await 前同步置位，持久化后才返回 true。新增两个 Bun 回归使用真实 JSONL，覆盖冷加载、草稿保持、并行 true/false 认领、元数据写回以及 awaiting/busy/已认领拒绝；加入 backend build 必跑列表。上游发布与既有补丁相同，待另行授权，不在本轮对外发布。

## 本地验证

- 核心 284 项通过，包含 3 项新纯策略/单飞测试；entities 模式另验证延迟旧读取不能影响重连后读取。类型化 facade 80 项操作的精确位置参数与返回类型全部覆盖，5 项 facade 测试通过。
- 应用 42 项通过，6 项资源测试默认忽略。模拟真实 WebSocket 验证 12 个顺序场景：持久等待/已认领/忙、不赢认领/旧 void 结果、发送拒绝、safe→allow-all ACK、直接批准、持久准备后 `/compact`、准备失败不发送、陈旧计划拒绝、clear 失败；send/clear 错误合并唤醒后不再请求。
- 显式真实后端 `persisted_plan_compacts_and_resumes_after_backend_restart` 通过：实际 SubmitPlan 与受限文件读取；短会话拒绝压缩时不派发；增加超过 SDK 保留窗口的真实会话历史后，手动压缩成功、pending ready；正常退出并启动全新后端，保留草稿快照，两次合并唤醒只发送一次批准消息，随后 clear 且再次唤醒不重复。该测试已加入三平台 native-check；使用明确的本地模型，不访问外部提供商。
- 重建完整后端及 contract bundle 成功，25 项相关 Bun 测试通过；真实控制器 recovery 契约通过。原生计划动作/历史只读/读取失败回归与已有 transcript 测试通过，另覆盖加载后虚拟行高度增长。
- fmt、全 workspace/all-targets/all-features clippy、389 个双语键与 35 根 codegen 检查通过；规格/规范两项独立审查及 P-06 增量审查通过。最终三平台、完整 E2E 与工程包状态见 PR 当前 head。

本地第一次后端重建因 8GB overlay 磁盘不足失败；迁移可再生 Rust 缓存后重跑成功。沿用上轮工具缓存，本地官方 ripgrep 下载限制与源码缓存来源见 [T-2-02](verification-extended-settings-2026-11-08.md)；锁定版本、正式下载和 CI 路径不变。完整上游 validate:ci 仍由 Linux contract CI 强制执行。

## 原生视觉记录

Linux Xvfb/lavapipe 实际运行 `cargo run -p agentkit-ui --features samples --example plans`。明确的独立组件夹具复现上传计划稿的正文/动作层级；不是实际执行 Job Object 任务，也不冒充完整工作台截图。

- [亮色计划卡](images/plan-card-light.png)
- [暗色计划卡](images/plan-card-dark.png)

阶段0/P0、物理 IME/辅助功能、参考机性能、Developer ID/公证继续开放。下一项 T-2-06 附件流程。
