# T-1-08 entities 与事件路由验证（2026-11-03）

基线 main `ce2508f`，PR #14 已合并；后端仍固定 mkagent v0.2.6。本轮把原生工作台从阶段0兼容投影迁移到正式生成 Session/Message、reducer、turns 与 GPUI entities。

`core::session` 维护消息/流/可见 turn 索引、缓存卡片、权限、会话草稿与待确认发送。普通已有流增量直接追加到对应消息/卡片，无历史克隆或重新分组；结构事件仅重组受影响尾段。首次流时间戳早于已有消息时显式使用排序回退。测试涵盖1000次 UTF-8 增量、历史前缀保留、全部162次上游 reducer 调用与完整 turns 投影逐事件一致。

可选 `entities` feature 提供全局 AppState、窗口 WorkspaceStore/SessionListStore、各打开会话 SessionStore。默认纯核心不依赖 GPUI。控制器每16ms收取最多256事件，按会话路由、合并通知；混合结构/增量批次仅发一次尾段重建，避免重复追加 Markdown。无效语义事件显示错误且不丢失同批有效事件。UI 读取实体，RPC/副作用和实体写入归控制器。

工作台保留每个会话草稿、后台处理和权限状态；发送先插入稳定 ID 的 pending 消息，确认/拒绝/丢失 ACK/服务端先回显/中断后迟到 ACK 均有处理。草稿修订号同时覆盖文本与附件，迟到确认不会清除后来编辑。附件存储与发送接口已就绪，附件选择 UI 属 T-1-14。会话快照加载及异步操作有代际屏障，加载/列表更新期间的有界事件缓冲会重放；更完整恢复体验仍属 T-1-21。

流式 Markdown 使用持久 TextViewState 追加；缓存结构改变时更新受影响列表尾段，保留读者滚动锚点。切换会话才重置滚动，重复绑定当前实体不重置。工具和最终答复使用正式 turns；更完整 ChatView/TurnCard 视觉仍按 T-1-12/13 推进。

本地验证：fmt、全 workspace/all-targets/all-features check/clippy 通过；核心测试255项、原生应用/UI测试21项、实际后端 Agent 契约1项通过。后者将每个真实 Read/流式/完成/取消事件输入缓存，并对照完整 turns 投影及最终持久化消息。两个上游 oracle 检查保持通过。LLVM 行覆盖：session 505/505、reducer 722/722、turns 638/638，三项均100%并设CI门禁；不代表100%分支或 region覆盖。

Linux 隔离 Xvfb/lavapipe 原生窗口冒烟使用实际控制器、真实 Bun/Pi 后端与本地固定模型，完成 Read 工具和最终流式答复并呈现窗口帧。该轮使用 debug 构建、128测试 token，性能工具按设计返回 invalid，原因包含 release-optimized-build-required / insufficient-frame-samples；只作为功能证据，不作性能或参考机验收。

两路独立审查核对原始规格与工程规范，修正旧加载回调提前清除 loading 屏障、后台发送失败不可见、抽取 helper 来源注释问题；新增回归验证。三平台最终 CI 与原生自动化窗口结果见本轮 PR。工程包仍保留 PR #6；真实签名、公证、物理桌面与参考机性能等阶段0门槛未关闭。

下一项 T-1-09：真实后端事件录制与14类场景夹具。阶段0尚未验收，本轮不构成发布验收。
