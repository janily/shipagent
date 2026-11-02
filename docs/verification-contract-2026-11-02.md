# T-1-04 客户端真实契约验证（2026-11-02）

基线 main `8dbe67f`（PR10 已合并），后端固定 v0.2.6。新增 `agentkit-client/tests/contract.rs`，CI 将 `agent-contract` 扩展为 `contract`，在 Linux、macOS15、Windows2022 显式运行客户端真实契约和既有完整 Agent 回合。

## 测试范围

| 范围 | 证据 |
| --- | --- |
| 实际打包 sidecar | 健康、切换工作区、会话创建/列表/查询/重命名/删除、草稿增读删、存附件/读内容、真实 bytes、类型化 name_changed 事件 |
| 实际 sidecar 能力路径 | file:openDialog → client:openFileDialog，内存测试回调验证参数、成功选择和错误传播；不打开原生对话框 |
| 同一固定上游 WsRpcServer 类 | 认证失败进入 Fatal、握手 metadata/频道/大小限制、嵌套及空 bytes、反向成功/失败/缺少能力/浏览器过滤/超时、并发响应、请求超时及迟到响应隔离、出站超限 |
| 同一上游真实重连/缓冲 | 透明代理断线；独立客户端离线推送，验证身份/顺序/seq 保留；超过导出 bufferLimit 后真实 stale；旧写失败且计数不重复，旧绑定句柄拒绝写入 |
| 既有真实 Agent 回合 | 本地 fake-llm 流式文本、Read 工具、结果持久化和取消继续运行 |

`transport.ts` 只提供测试 handler。构建读取同一份固定源码和锁定依赖，原样使用上游 transport/codec/buffer 策略，不改生产 handler、不改私有 buffer、不伪造 ack 或事件。产物单独位于 `target/contract/<平台>`，不进入发布 backend。普通 backend build 不构建它。

## 测试发现与修复

真实服务发现 Rust 能力失败发送 `type:error`；上游 `onClientResponse` 只在 `type:response` 上关联 `error`。因此成功能力回调可用，失败/超时却等到服务端超时。回归先复现真实等待超时及精确帧类型断言失败，再统一将反向错误封装为 response.error；未知、busy、panic、timeout、显式错误和超限替代响应均保留 id/channel/code。请求侧业务错误处理不变。

本地 3 个真实客户端契约、1 个半握手清理回归与既有真实 Agent 测试通过；核心75项、能力5项包含回归断言。全量核心/原生/Clippy/fmt 与最终三平台 CI 作为提交门禁，最终结果以 PR 当前提交检查为准。

本地完整 backend 重建遇到 ripgrep 下载限制。向安装脚本注入 GitHub 凭据被自动审批拒绝；采用无凭据、禁用安装脚本的隔离锁定依赖构建测试 fixture，并使用已验证 backend 工程产物运行本地业务契约。CI 使用现有后端构建流程验证 `backend build --contract` 的完整构建。

## 保留边界

测试能力由内存 handler 实现；原生窗口/授权 UI、自动快照恢复与所有阶段1事件夹具仍归各自任务。完整 U1–U6 应用端到端流程尚未完成。阶段0仍未通过；物理桌面/性能/真实签名、公证验收未完成，PR6工程包继续保留。

规范/规格两轴独立审查均无阻塞问题；规范审查指出握手阶段的代理线程可能在清理时挂起。半握手用例先复现清理超时，再将连接设置纳入停止信号与截止时间，回归通过。
