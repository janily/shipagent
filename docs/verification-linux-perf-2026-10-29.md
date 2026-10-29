# Linux 原生流式性能测量

日期：2026-10-29。基线：main 2d00381；Rust 1.99.1、GPUI Kit 0.8.1、mkagent v0.2.6、自带 Bun 1.3.14。本轮补齐 T-0-05 测量工具，扩展 T-0-08 固定速率与故障脚本；不宣称阶段 0 整体验收通过。

## 测量条件

真实 release 构建，opt-level=3、thin LTO，开启可选 perf/GPUI profiler；正常构建不启用。云容器 cgroup 限制 2 CPU、8 GiB；Xvfb 1440×960、Mesa lavapipe 软件 Vulkan，实际 Workbench 窗口 1280×820。Python subreaper 监督每次独立运行，xdotool 明确聚焦窗口；正式三组原生更新的激活比例均为 100%。具体 CPU/内核/限制与 binary SHA256 随 JSON 保存。

后端与模型连接均在私有临时目录，真实链路为：原生 controller → WebSocket → bundled backend → Pi → 本地模型 → 实际 Read → 回传工具结果 → 600 个固定测试模型 token 的 Markdown。固定 tokenizer 为“空白分隔词及其尾随空白”，输出 6292 字节；不是把字符或 SSE 包当作厂商 BPE token。源按单调时钟 deadline 排程 50 token/s，记录实际速率和最大间隔。

采集从首个文本事件开始，覆盖中间答复、工具、最终文本及完成后 100ms 尾部，不丢弃预热区间。时间来自实际 GPUI Draw/Present，另测 controller 更新和批次接收到呈现的延迟。完整定义与复现命令见 [工具说明](../tools/perf/README.md)。

## 正式三组结果

下表时间单位 ms，均为 p95；帧数为测得的 GPUI frame-work 样本。

| 运行 | 源速率 token/s | 有效帧数 | GPUI frame work | Draw | 提交 | Dirty→Present | 批次接收→Present |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 50.000 | 248 | 5.51 | 4.98 | 52.56 | 64.01 | 110.42 |
| 2 | 50.015 | 220 | 6.35 | 6.03 | 61.44 | 71.39 | 125.64 |
| 3 | 50.001 | 232 | 9.07 | 6.58 | 58.11 | 69.33 | 125.06 |

三组均为有效测量，主线程 frame work p95 低于 16ms；每组都有完整工具/终止证据、最终 6292 字节与已提交的完成帧、正常退出且无所属残留进程。**这不等于整体流畅度/低延迟达标**：软件渲染提交 p95 为 52–61ms，批次接收到呈现为 110–126ms；尚未满足“一帧更新”预期，也未验证 60fps。当前主要观察到软件呈现/调度的开销，不能把仅有 5.51–9.07ms 的 frame work 包装成完整产品性能通过。

该机器不是规格指定的 Ubuntu 22.04 i5-8250U 8GB 基准机，故所有报告的 referenceAcceptance 均为 not_evaluated。参考机/真实 GPU 上的预算和延迟仍需验证。正式 T-1-23 的 nightly 对比与 10% 退化告警也未在本次冒充完成。

## 故障与无效样本

- 模型在最终输出第 50 个测试 token 时断流：原生 outcome=failed，报告 invalid，正常回收后端，无强制清理和残留。快速帧不能使失败回合通过。
- 原生进程在 2 秒后被强制终止：报告 invalid，监督器收养并回收后端/Pi 两个孤儿，remainingOwnedProcesses=0，forced=true。该运行不作为性能成功样本。
- 调试版冒烟确认真实窗口及报告闭环；因非 release 和样本不足被拒绝。
- 初始未验证焦点的三组数据仅作诊断保留。发现 Xvfb 无窗口管理器后，新增 ≥95% 原生激活更新门槛并重新测量，未从旧数据挑选更快样本当正式结果。

## 自动化与审查

- 37 项核心 Rust、7 项原生（5 控件 + 2 指标）、11 项 Node、3 项 Linux 进程监督测试通过；fmt、clippy -D warnings、依赖审计通过。
- CI 仍仅针对 Linux；原生作业启用 perf feature 验证真实采样代码，契约作业运行负载/判定/监督测试。另有手动软件渲染实验工作流，上传原始报告而非作为参考机门禁。
- 独立审查发现裸 PID 轮询会覆盖旧身份、漏掉父进程退出后的孤儿。已删除该扫描方案，改用 Linux CHILD_SUBREAPER 的内核子进程归属，等待回收前 PID 不可复用；父进程刚生成子进程即退出、SIGTERM 活动父进程、无关进程不被清理三项回归通过，真实原生强杀亦验证回收。
- 本环境内核没有 /proc/.../children 文件，监督器回退为读取 PPID 等于自身的直接子进程。监督器自身持续存活且持有内核子进程身份，未恢复为不可靠的 app-PID 快照轮询。

## 证据

- [三组聚焦正式数据](perf/linux-stream-50.json)
- [模型断流](perf/linux-model-disconnect.json)
- [原生强杀与孤儿回收](perf/linux-native-crash.json)
- [未聚焦诊断原始记录](perf/linux-unfocused-diagnostic.json)
- [运行中的原生窗口](images/perf-native-release-stream.png)

后续：T-0-10 Linux 组件样板；参考 GPU/机型延迟与帧预算、万条消息/大工具输出等性能用例、阶段报告继续补齐。macOS 与 Windows 的平台专项验证按用户要求最后进行。
