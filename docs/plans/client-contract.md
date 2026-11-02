# T-1-04 真实客户端契约测试计划

基线：main 8dbe67f（PR10 已合并）。规格来源：05 测试层级、06 T-1-04、02 协议。仅测试与测试构建/CI；不扩展生产频道、平台能力或恢复 UI。

1. 构建隔离的上游 WsRpcServer 契约夹具。复用固定后端 disposable source 和锁定依赖，测试 handler 使用真实 transport/codec。产物在 target/contract/<platform>，不进入可发布后端。普通 backend build 不构建测试夹具。
2. client/tests/contract.rs：真实 TCP + Bun 上游服务覆盖认证/握手 metadata、双向嵌套 bytes、异步能力成功/失败/超时、RPC 超时晚响应隔离、真实事件重放/stale/代际写禁止重放；故障代理只断连接和暂停重连，不伪造协议帧。另启动实际 SidecarHandle 验证工作区/会话/草稿/附件 CRUD 及事件。保留现有真实 Agent 流式 Read/持久化/取消测试。
3. 三平台 CI contract job 构建真实 backend 和夹具，显式运行 ignored contract 与 agent_turn。补开发命令和精确覆盖说明。核心、真实契约、native、clippy/fmt/drift，标准与规格分开审查，commit/push/PR，不自动合并。

边界：上游未开放可触发全部反向能力和 buffer 溢出的业务 handler，因此这些测试使用同一固定 WsRpcServer 类、原样 codec 与 buffer 策略，测试 handler 独立构建；不得宣称所有行为都通过发布 sidecar 业务 API 触发。U1–U6 原生 UI/e2e 及阶段1全部事件场景属于后续UI/fixture任务。
