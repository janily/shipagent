# 阶段 0 风险报告与后续工作量估计

日期：2026-10-31。基线 main ad5083a，后端 mkagent v0.2.6 / 6e8ebc456b6b82441be09118ca11afca79b84836。结论：**阶段 0 未通过整体验收**。这是 T-0-11 的中期风险报告，不是通过报告。

用户已恢复三平台验证，随后确认签名 Secrets 尚未配置，先保留工程包结果，并要求继续开发。因此独立推进依赖 T-0-07 的 T-1-01；此顺序调整不豁免阶段门槛，也不启动发布。PR #6 尚未合并，本分支从 main 开发，不包含其打包实现。

## 已有证据

| 工作 | 结果与证据 | 结论范围 |
| --- | --- | --- |
| T-0-01–04、06 | [首次验证](verification-2026-10-29.md)、[真实原生对话](verification-native-turn-2026-10-29.md) | 固定后端构建，RPC、Read 工具、流式对话、持久化和取消已有实际证据；正式重连/能力分派未完成 |
| T-0-07 | [生成可行性](verification-codegen-2026-10-29.md) | 三类核心协议生成可行，正式生成另由 T-1-01 加固 |
| T-0-05、08 | [Linux 测量](verification-linux-perf-2026-10-29.md) | 三次 600 测试 token，固定 50 token/s；断流/强杀脚本已有，非完整性能通过 |
| T-0-10 | [组件及三平台验证](verification-components-platform-2026-10-29.md) | 六类 Kit 样板，Linux X11 / macOS arm64 / Windows x64 实际链接、后端契约、窗口提交；不等同物理桌面端到端验收 |
| T-0-09 工程包 | [PR #6](https://github.com/janily/shipagent/pull/6)、[固定提交报告](https://github.com/janily/shipagent/blob/d936cef81b971df3a9df82e2efb5da4b6623ada3/docs/verification-macos-package-2026-10-31.md)、[流水线 #4](https://github.com/janily/shipagent/actions/runs/41193327453) | ad-hoc + hardened runtime DMG；挂载后独立运行和真实 Agent 契约通过，源后端移走后仍可运行；尚未合并 |

工程 DMG 68,024,879 字节；SHA256 `40a65326b770f450b0badd66482c8d7fdf6818ff3856dbd53386b5abe9613366`。[工程产物](https://github.com/janily/shipagent/actions/runs/41193327453/artifacts/13039148405) 有 GitHub 保留期限；固定提交报告保留测量结果。

## 未关闭风险与门槛

| 项目 | 当前事实 | 关闭条件 / 后续任务 |
| --- | --- | --- |
| macOS 发布信任链 | 无 Developer ID / 公证凭据；当前只有 ad-hoc 工程包 | 配置后真实签名、公证、staple、Gatekeeper 与干净机验证；T-0-09 / T-1-22 |
| Bun JIT 最小权限 | ad-hoc 下 none / jit / jit-unsigned 三种策略均通过 DFG 与 Wasm 探针；不能据此证明发布签名策略 | Developer ID 签名后重新比较；暂保留只给 Bun 的两项 JIT entitlement |
| 参考机性能 | Linux 为 2 CPU / 8GiB + lavapipe 软件 Vulkan；主线程 work p95 5.51–9.07ms，提交 52–61ms，接收到呈现 110–126ms | 参考硬件/GPU 测 50 token/s、帧预算和输入延迟；不得标记 60fps 或单帧延迟达标 |
| Windows 崩溃清理 | 契约正常退出通过，Job Object 尚未实现 | 强杀主进程验证 sidecar/Pi/工具无残留；T-1-05 / T-3-01 |
| 原生交互及平台覆盖 | 自动化不能证明物理屏幕、IME 候选、辅助功能；macOS Intel / macOS 13、Windows arm64、Linux Wayland 没有本轮证据 | 支持矩阵对应实机验收，区分组件样板与主应用完整流程 |
| 正式应用完整性 | UI/reducer 仍是阶段 0 最小投影 | T-1-02–21 和 U1–U3/U6 验收，不能把原型称为完整工作台 |

## 工作量重估

06 页阶段 1 原估算 135 人日。不能从会话或 CI 运行时长推算团队速度；下面是按尚未关闭风险给出的工程估计范围，不是交付日期或已承诺排期。T-1-01 完成后名义剩余 130 人日，其余原型不提前抵扣正式任务。

| 模块（任务） | 原估剩余人日 | 当前估计人日 | 依据 |
| --- | ---: | ---: | --- |
| client/API/能力/契约（02–04、19） | 21 | 22–30 | 全量重连、seq/stale、能力与二进制边界仍待完整实现 |
| sidecar/异常恢复（05、21） | 8 | 10–14 | 三平台生命周期、Windows 崩溃清理和 shell 环境 |
| reducer/turns/entities/夹具（06–09） | 29 | 29–38 | 原型未覆盖上游全部移植测试和 14 个场景 |
| 应用、会话、输入、设置与引导（10–18、20） | 59 | 59–74 | Kit 可行性已验证，正式状态管理/持久化/交互仍未交付 |
| 发布、性能及 P0 验收（22–24） | 13 | 13–20 | 发布凭据、参考机测量、实际用户流程 |
| 合计 | 130 | 133–176 | 不是可直接相加成自然周的单人排期 |

阶段 0 剩余验证另预留 3–7 人日（不含等待凭据、硬件、人工测试时间），合计约 136–183 人日。阶段 2–4 暂保留原 WBS 作为基线，待阶段 1 完整模块和实际团队速度出现后再重估，避免给尚无证据的精确数字。

下一项按依赖推进 T-1-02 完整 client，再接 T-1-03 API 门面与 T-1-04 真实契约；并行开发可能性不构成阶段 0 验收通过。签名、公证和参考机结果补齐后，更新此报告并逐项给出通过/失败结论。
