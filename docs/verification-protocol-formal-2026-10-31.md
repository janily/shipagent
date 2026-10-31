# T-1-01 正式协议生成验证

日期：2026-10-31。基线 main ad5083a，后端保持 6e8ebc456b6b82441be09118ca11afca79b84836，未改子模块源码。本分支独立于未合并的 PR #6。

## 交付范围

- 显式 14 类核心根类型及可达字段；生成器读取公开类型导出，固定提取器/TypeScript/Cargo lock；HEAD、gitlink、清单 pin 一致且子模块干净才生成。
- `overrides/schema.json` 精确匹配原 schema 后才修正：Message/Session/Workspace 时间戳 i64、附件字节 u64、messageCount u32。保留 elapsedSeconds 等未经审查的 TS number 为 f64；不声称全部数值域已人工建模。
- 相同嵌套形状共享命名类型，使 override 同时适用于 Session、事件和独立 DTO；40 种已知事件 type、41 个具名分支（retry 两种形状），不再使用 VariantN 业务接口。
- 静态读取完整 RPC_CHANNELS、协议版本和四个消息大小常量；当前 WorkbenchApi 已使用生成常量。
- 有效 schema、来源摘要、生成 Rust/常量进入 `codegen-drift`。`--check` 拒绝缺失、变化和遗留 schema，并不写文件；失败的提取/override 不发布部分结果。
- 真实对话 prototype 入口现在经过 DecodedSessionEvent 验证；已知损坏载荷不能转成 unknown 放行，未来未知事件/新增字段仍无损保留。

完整 API 门面、状态模型和 reducer 移植仍是 T-1-03/T-1-06；本轮不把 prototype Session/Message 投影视为最终 wire DTO。连接设置/OAuth 等含凭据类型需在 API 门面阶段设计秘密包装，未批量生成 Debug 明文凭据结构体。全部频道被生成不代表全部功能已实现。

## 验证

本地 Linux：

- Node 5 项测试：override 原值变化/缺失/重复拒绝，联合顺序无关命名，新 retry 形状拒绝，AST 受限表达式及重复键拒绝，来源锁 guard，重复提取逐字节一致和产物 SHA256。
- 核心 Rust 43 项通过；默认 suite 中两项真实后端测试保留 ignored，显式契约另行运行。9 项 DTO/事件测试覆盖整数边界、负 epoch、分数秒、语义枚举、required/nullable/optional 集合、新增字段及 unknown 事件。
- 实际入口损坏 text_delta/retry/user_message 回归先失败，再通过。旧 store 夹具缺少真实必填 timestamp，原生 UI 工具结果夹具缺 toolName，均已按上游补齐，没有放宽 wire schema。
- 遗留 schema 检查回归先失败，再通过；`--check` 对修改/缺失产物不写入的既有测试继续通过。
- bundled backend + Pi + 本地 SSE 模型显式真实 Agent 契约通过：Read、文本流、最终持久化、取消，全部事件和会话快照通过生成类型。
- 全功能 workspace clippy `-D warnings`、fmt、codegen --check、diff 检查通过；原生 UI/App 10 项测试通过。

初次 [CI #30](https://github.com/janily/shipagent/actions/runs/41252148749) 的三个 macOS 作业在启动前被 GitHub macos-14 runner 计划停机取消（[官方退役公告](https://github.com/actions/runner-images/issues/13518)，2026-11-02 退役）。当前 CI 矩阵迁到 macos-15，重新验证实际平台；旧报告的 macos-14 结果仍只属于旧提交。最终三平台 CI 链接补于合并前；不得借用 PR #6 或旧提交的成功状态作为本分支结果。当前没有新增物理桌面、性能或正式签名验收结论。

## 后续

T-1-02 完整 client 重连/seq/stale/能力/超时/大小检查，再推进 T-1-03 门面。阶段 0 发布签名与参考机等风险仍未关闭，见 [中期风险报告](phase-zero-risk-report-2026-10-31.md)。
