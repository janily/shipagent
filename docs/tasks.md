# 任务进度

2026-10-29 用户调整：先针对 Linux 开发验证，macOS 与 Windows 平台验收放到最后。既有 macOS CI 记录不等于应用打包/干净机验收；当前新增 CI 仅运行 Linux。

以 [06 任务分解与排期](spec/06%20任务分解与排期.md) 的 T-阶段-序号为唯一任务编号。原文人日是项目估算，不代表本次会话耗时或完成进度。

| 任务 | 状态 | 已有证据 / 剩余内容 |
| --- | --- | --- |
| 规格与原型核对 | 完成 | 00–07 页入库，五张 HTML 页面静态解包核对 |
| T-0-01 仓库骨架 | 已实现，CI 结果另见 GitHub | workspace、工具链、依赖锁、品牌、CODEOWNERS、lint/unit/native-check；本地 fmt/clippy/审计/全workspace检查通过 |
| T-0-02 后端子模块/补丁/构建 | Linux 验证通过，macOS 待验 | 固定 v0.2.6；P-03；隔离临时源码构建；有自带 Bun/Pi/资源，无 WebUI；未跑 macOS 构建 |
| T-0-03 sidecar 原型 | Linux 冒烟通过 | 内存随机 token、就绪验证、异步启动、Unix进程组退出；正式重启策略等归 T-1-05 |
| T-0-04 RPC 原型 | 本地传输与真实健康往返通过 | 21项核心测试覆盖部分协议/传输/生命周期/构建；真实健康及工作区读取另显式通过；真实发送/工具回合已另通过；修复 seq_ack 非空 id |
| T-0-05 | Linux 窗口及测量工具完成；参考机预算/延迟待验 | release 实测 3×600 测试 token；work p95 5.51–9.07ms，但软件提交 52–61ms、批次呈现 110–126ms，不标记整体性能通过 |
| T-0-06 | Linux 真实链路通过 | 原生流式窗口、Read 工具回合、最终持久化及取消 |
| T-0-07 | Linux 可行性验证通过 | TS→JSON Schema→Rust 生成 SessionEvent/Message/Session，真实后端解码；codegen --check 不写入防漂移；正式类型迁移归 T-1-01 |
| T-0-08 | 最小版及 Linux 固定负载/故障脚本完成 | 50 个明确定义的测试 token/s、真实 Read、断流及原生强杀；更广夹具归 T-1-09 |
| T-0-09 | 按用户要求后置 | 需要 macOS、签名与公证凭据、干净机 |
| T-0-10/T-0-11 | 未完成 | 组件样板；全部验证后才能产出阶段0通过报告 |
| 阶段 1–4 | 未开始 | 按第06页依赖与阶段门槛执行 |

开发命令见 [development.md](development.md)，实测和限制见 [首次验证](verification-2026-10-29.md) 与 [原生对话验证](verification-native-turn-2026-10-29.md)。当前不标记阶段0通过，不发布空界面应用。

本轮生成证据与兼容处理见 [T-0-07 验证](verification-codegen-2026-10-29.md)。

本轮性能范围与未达标项见 [Linux 流式测量](verification-linux-perf-2026-10-29.md)。
