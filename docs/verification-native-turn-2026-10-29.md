# T-0-05 / T-0-06 原生流式对话验证

日期：2026-10-29。基线：main 9955575。环境：Linux x64；Rust 1.99.1、GPUI Kit 0.8.1、mkagent v0.2.6、Bun 1.3.14；Xvfb + Mesa 软件 Vulkan。以下结果不替代 macOS 干净机或性能基准。

## 实际链路

原生 Workbench → WorkbenchApi → 认证 WebSocket → 自带 Bun 的真实 mkagent → Pi Agent → 本地 OpenAI-compatible SSE 模型 → 真实 Read 工具 → 第二次模型请求 → 最终 Markdown → 后端持久化。

fake-llm 只发送工具调用，不读取文件。第二次请求必须包含实际工具返回的验证标记，否则返回 422。原生窗口手动输入发送后，模型统计出现 toolCalls=1、toolResults=1、errors=0；工具卡展开显示真实 fixture 文件路径和内容。UI 没有注入示例回复。

## 验证

- 29 项核心 Rust 测试通过；默认 suite 另有 2 项真实后端测试 ignored，必须显式执行。
- 5 项 GPUI Kit 无头窗口测试通过：中文输入发送/停止、流式与最终文本/工具行、发送失败保留草稿、显式拒绝及重复点击保护、加载期间 Enter 不提交且失败后可重试。
- Node fake-llm 2 项测试通过：SSE 工具流程、缺少工具证据拒绝。
- 真实后端 agent_turn 显式通过：accepted → text_delta → tool_start → tool_result → complete；最终答复和单个用户消息持久化。第二个回合收到首个 delta 后取消，返回 interrupted，后端 isProcessing=false。约 5.35 秒，非性能基准。
- 原生 Linux 可执行程序构建、全 workspace/all-targets clippy -D warnings、fmt、cargo deny 检查通过。审计仍有上游维护状态和未用许可证允许项提示。
- 真实窗口检查：连接表单 → 会话 → 流式工具回合 → 展开工具输出；浅色与深色主题使用同一组 Kit 语义角色。

## 实测暴露并修复的问题

- 上游 codec 要求所有 envelope 都有非空 id；旧 seq_ack 无 id，导致连接在第 5 秒断开。新增真实 WebSocket 回归测试，确认 ack 有 id 且仍可 Pong/后续 invoke。
- 加载时 Enter 不得进入 submitting；加载失败保留 controller 旧会话；error 后等待 complete 才解除忙碌，防止并发发送。
- SessionEvent 序列化保留原始未知字段；有往返测试。
- GPUI 退出回调只有 200ms，不能在其中等待 5 秒清理。正常退出先等待保留的启动任务、停止会话并回收后端，再退出；启动中关闭也等待启动任务清理。系统退出回调使用短期限兜底。

## 范围及剩余门槛

实现为阶段 0 原型：单工作区、单活跃回合；运行期间禁止切换会话。完整多会话后台、队列、自动重连、持久草稿、OAuth/完整设置、计划/文件/后台任务面板仍按后续任务执行。连接表单目前是 OpenAI-compatible 端点，保存凭据交由后端；不表示全部供应商引导已完成。

后端当前的持久化聊天快照不含本次 live tool 行；重开会话能恢复用户和助手答复，工具卡只保证当前实时事件视图。工具历史恢复需在完整事件归约/后端集成阶段处理，不把实时工具 UI 当作持久工具历史。

T-0-07 完整 DTO/codegen 可行性、T-0-08 更广的 50 token/s 故障脚本、T-0-09 macOS 签名公证与干净机、T-0-10 更广组件样板和 T-0-11 阶段报告仍未全部完成。当前不标记阶段 0 整体验收通过。

Cargo.lock 为 Kit test-support 同时解析其可选 gpui-fast 依赖，将共享 taffy 固定到 0.15.0、unicode-properties 到 0.1.3；Kit=0.8.1 与实际 gpui-pre=0.3.10 未变。新增可选平台依赖也纳入依赖审计。
