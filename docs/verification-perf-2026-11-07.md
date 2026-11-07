# T-1-23 原生性能基准与 CI

基线为 PR #29 合并后的 `e20c2a5`，后端仍固定 v0.2.6。实现与测量边界见 [运行指南](../tools/perf/README.md) 和 [计划](plans/performance-ci.md)。本任务建设可重复的工程测量与告警，不关闭阶段0或参考硬件验收。

## 实现

真实 release 工作台通过既有 controller、认证 WebSocket、固定后端与 Pi 执行 Read，然后由本地模型按真实单调时钟输出 50 个明确定义的测试 token/s。首启偏好只写入隔离目录，明确延期引导；不会操作个人配置。记录实际 GPUI Draw/Present、流式更新、首帧和后端/夹具就绪观测。

独立 release 原生场景使用正式 WorkbenchView、SessionStore、ToolCard、ToolOutputView 和 FullToolEditor，加载十个百条会话、万条 Markdown 历史、120 次确定性滚动和十万行工具结果。RSS 是该 UI 场景进程的快照，不含完整应用 controller/RPC/偏好/设置实体，不能据此验收生产进程内存预算。历史打开包含构造/索引/绑定与首次提交，不含磁盘/网络下载；滚动是程序化位置更新的实际提交频率，不是触控板延迟。

每项至少三次完整运行，保留全部原始数据并取中位数；任何缺失、无效构建、未完成回合、清理失败、节奏/内容/帧覆盖率不合格均使聚合无效。未配对 Present 计入缺失，帧计数必须为非负整数且总数一致。缺失数据不按零处理，也不会显示预算通过。

夜间、手动及相关 PR 在共享 Linux/macOS 构建 release 并上传报告。比较对象只选前一个 main schedule，包括失败/尚未完成的运行；手动执行不替代前夜，也不回退寻找更早的有利数据。硬件、cgroup、工作负载、窗口或测量源码指纹不同则不比较。耗时/内存增加或 FPS 降低严格超过 10% 告警。首次没有前夜基线也明确告警。

Linux 保留内核 subreaper 所有权监督。macOS 正常退出记录实际 owned sidecar supervisor 的关闭结果；这不是 orphan census。异常时先 TERM，宽限后 KILL 原生，停止后续样本/场景。可选参考任务要求 `disposable-machine-per-job` 运维合约和每次作业后销毁 OS，禁止持久个人桌面；配置标签本身不是物理 M1 验收证据。

## 验证

- 基线核心测试 280 项通过；当前带 perf 的应用单元测试 40 项通过，3 项原有原生/子进程辅助入口保持显式忽略。
- Node 直接逐文件执行 25 项通过，覆盖 pacing、完成/断流、环境隔离、清理证据、指纹、三次聚合、前夜选择、阈值和缺失数据。Linux supervisor 3 项通过。
- 当前 app/UI 全特性全目标 clippy、fmt、i18n 329 键检查、performance workflow actionlint 通过。
- 两项独立规格/规范审查完成；修复帧覆盖率假阳性、手动任务替代前夜，以及 macOS 异常后继续测量/隔离不充分的问题。

## 保持开放的门槛

所有报告保留 `referenceAcceptance: not_evaluated`。共享 Xvfb/lavapipe 和 macOS 虚拟显示不能替代物理参考机。app-entry 启动观测排除了加载器，夹具就绪包含模型端点配置，尚未建立真实冷启动到可输入的时间。控制器批次收到至后续提交不是网络事件延迟。完整生产进程空闲/十会话 RSS、物理显示/输入延迟、IME/无障碍、七天稳定性、Developer ID/公证/干净机验收继续开放。

下一项 T-1-24：端到端 U1–U3、U6 与 P0 验收，继续按实际证据保留未达标项。
