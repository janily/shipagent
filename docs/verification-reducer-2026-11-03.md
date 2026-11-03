# T-1-06 reducer 移植验证（2026-11-03）

基线 main `37f9636950b59e2788f629efdca320adbfcd08b2`（PR #12 已合并），上游固定 mkagent v0.2.6。新增公开 `agentkit_store::core::reducer`，使用正式生成 Session/Message/SessionEvent、Streaming、Effect；借用不可变输入并返回新状态与延后执行的效果。显式 Context 注入时间与消息 ID，代替上游全局时钟/计数器；嵌套生成 DTO 适配失败、非整数/超范围消息时间戳返回类型化错误。

移植包含文本/权威消息 ID、重试、工具/后台任务、完成/失败/中断、队列用户消息、权限/编码模式效果、元数据、批注和上下文用量。外来会话事件与未来事件标签不修改当前状态，也不产生当前会话效果。未来原始字段保留在 DecodedSessionEvent 入口。

`tools/reducer/oracle.mjs` 实际执行上游八个文件的 31 项原始测试，捕获每次 processor/handler 调用；另有 20 个补充场景，共 162 次调用。来源哈希、生成器输入哈希和原始断言随 fixture 保存，CI 执行只读漂移检查。Rust 按八个对应模块对照完整状态/效果，检查输入不变、确定性和字段未静默丢失；见 [oracle说明](../tools/reducer/README.md)。上游 `as any` 测试缺失的必需 wire 字段仅在夹具补齐；旧 `annotations[].motivation` 是唯一显式兼容例外，生产生成模型未放宽。

真实 Agent 契约同时馈入正式 reducer：校验流式最终答复 ID/内容与持久化查询一致，Read 工具 ID/结果与实时事件一致，取消后停止处理并清空流。后端精简历史主动省略工具消息，因此工具结果不与持久化工具行做不存在的对照。

审查修复包括空 turnId 的 JavaScript truthy 匹配语义和工具错误前缀的 ECMAScript 空白集合；补充原始上游 oracle 场景先验证失败再修复。移植函数逐一注明上游来源和 tag。

本地 fmt、全 workspace/all-targets/all-features check 与 clippy 通过；核心测试135项、原生UI/应用测试13项及真实Agent契约1项通过。LLVM测得新增正式reducer五文件行覆盖率713/713（100%）；region覆盖率另保留原始报告，未宣称100%分支覆盖。CI上传覆盖率JSON并强制新增reducer的行覆盖率100%。结果记录于本轮证据目录。三平台 CI 最终结果以 PR 检查为准；不把 CI 实窗提交视为物理桌面验收。

正式 reducer 尚未接管 GPUI 状态；T-1-07 实现 turns，T-1-08 集成 entities/同帧合并/快速路径/乐观发送。阶段 0 仍未通过，签名/公证、参考机性能和完整恢复 UX 门槛不变。
