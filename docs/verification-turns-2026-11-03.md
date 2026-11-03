# T-1-07 turns 移植验证（2026-11-03）

基线 main `33485d5`，PR #13 已合并，上游仍固定 mkagent v0.2.6。新增公开 `core::turns`，直接接收正式生成 Message，输出有明确判别标签的 Turn/AssistantTurn/Activity/ActivityGroup；不引入 GPUI 或时钟、网络、文件 IO。

实现用户/计划/最终回复边界、隐藏消息过滤、稳定时间排序、pending/intermediate/streaming/complete 阶段、状态压缩和中断、计划批注、最近 todos、子 Agent 深度/循环上限/跨 turn continued 分组、TaskOutput 用量合并、稳定 UI key、摘要/末尾选择器、duration/tokens 和带显式历史偏移的时间格式。JS truthy、空白、Number.toFixed 舍入和大数最短显示等边界通过实际源函数对照。

七个上游文件共81项原始测试均实际执行：五个 turns 文件、一个时间格式文件、一个 React 计划按钮样式源文本检查文件。79项纯逻辑用例有六个对应 Rust 模块；另有完整用例归属检查、16个补充场景和3个边界测试，共237次源函数调用。React 两个源文本断言仍运行并锁定来源哈希，**不把它们冒充为 GPUI 按钮验收**。原生计划按钮属于后续 UI 任务。

[oracle说明](../tools/turns/README.md) 记录确定时钟/随机数/UTC、来源哈希、完整参数/结果、Set 规范化和字段审计。生产生成模型未放宽。工具 todo 的非法视图数组不渲染；原始消息和 toolInput 不被修改。时间格式显式接受目标时刻的本地偏移，DST/本地时区解析留在应用边界；不依赖服务器当天时区。

真实 Agent 契约把正式 reducer 输出接入 turns：检验实际 Read 运行时的 tool_active 阶段、最终回复 ID/内容与持久化查询一致、取消后卡片 complete。没有提前迁移 GPUI Entity、同帧合并或流式快速路径，这些仍归 T-1-08。

本地 fmt、全 workspace/all-targets/all-features check/clippy 通过；核心测试234项、原生UI/应用13项、真实Agent契约1项通过。LLVM测得正式 turns 行覆盖638/638（100%），reducer保持713/713（100%），两项均由CI强制。审查发现的signed-zero差异已先复现失败再修复，新增显式负零夹具。证据记录于本轮目录；三平台最终结果见 PR 检查。两路审查分别核对工程规范和原始规格/上游行为。100%行覆盖不代表100%分支/region覆盖。

下一项 T-1-08 接入 entities、事件路由、同帧合并、流式快速路径和乐观发送。阶段0尚未验收，物理桌面、参考机性能、真实签名/公证等原有门槛不变。
