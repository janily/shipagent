# T-0-07 Linux 协议代码生成验证

日期：2026-10-29。基线：main d2e3bdc，后端固定为 mkagent 6e8ebc456b6b82441be09118ca11afca79b84836。按用户最新要求，本轮只针对 Linux；macOS 与 Windows 平台验收延后，不视为已通过。

## 已交付

- 从上游 protocol/index.ts 与 core/types/index.ts 的导出入口提取 Message、Session、SessionEvent 及其可达字段，使用锁定的 ts-json-schema-generator 2.9.0。
- `cargo xtask codegen` 通过 typify 0.6.0（lock 中实现为 0.6.2）生成 Rust；schema、生成代码和来源版本入库。
- `cargo xtask codegen --check` 在临时目录生成，再比较全部产物；缺失、修改或上游源漂移均报错，不改写待检查文件。
- generated 类型与现有 UI prototype 边界并行，真实后端契约同时验证两者；新增 session_generated API 获取实际 RPC 快照，避免拿裁剪后的 prototype 字段冒充原始后端数据。
- Linux CI 增加 codegen-drift；原 native-check 改为 Ubuntu 安装原生依赖后检查全部 workspace，并跑 GPUI 无头控件测试。Mac/Windows 重新启用前仍需单独平台验收。

## 测试与证据

- 37 项核心 Rust 测试，包括 6 项新 DTO 契约测试、1 项 CLI 帮助和 1 项不写入漂移检查测试；默认 suite 中 2 项真实后端测试仍 ignored，必须显式运行。
- 生成契约覆盖 required 字段、optional missing/empty、nullable required、所有测试事件判别、两种 retry 形状、allow-all 拼写、新增字段兼容和未知事件无损往返。
- Node 提取测试重复生成并逐字节比较，核对上游 timestamp/workspaceName 必填和工具/retry 形状。
- 实际 bundled backend + Pi + 本地 SSE 模型：真实 Read 工具回合、最终会话/Message 持久化、首个 delta 后取消。所有收到的事件以生成 DTO 验证；会话快照直接从真实 RPC 解码。
- 原生 GPUI 5 项回归测试、Linux 全 workspace 检查、fmt、clippy -D warnings 和依赖审计；GitHub CI 结论以 PR 检查结果为准。

## 发现与兼容处理

1. 上游公共入口包含两个不同的 PermissionRequest 声明。提取使用 expose:none 展开目标类型的依赖结构；完全相同的 Message 结构改为共享引用，避免重复或错误合并。不同同名定义会使 Rust 生成失败。
2. typify 对 JSON Schema const 的部分处理会退化为普通字符串，使错误事件混入其他分支。生成前将 const 转为语义等价的 singleton enum；两种 retry 的 phase 和必填字段有失败到通过的回归测试。
3. typify 默认把 optional 集合的缺失和显式 []/{} 合并。Rust AST 转换将这些字段保留为 Option，仍保持 required Vec/Map 必填。
4. 独立审查发现 required nullable 的 model/usedTokens 被 Serde Option 当作可缺省。已加反序列化钩子，显式 null 有效、缺失报错；回归测试先复现失败再通过。optional 字段按规格指定的 Serde Option 语义接受 null 为 None。
5. DecodedSessionEvent 保留完整 raw payload；已知 type 必须通过生成 DTO 验证，不能因字段错误转入 Unknown；未来 type 可以保留并往返。

## 可行性结论与边界

T-0-07 的三类协议生成并解析真实事件在 Linux 上可行。后端源码和版本未修改，无需安装或执行整个后端应用依赖树来生成 schema。生成器跳过的是公共入口引入的无关 TypeScript 应用诊断，不声称已完成上游全项目 typecheck。

TS number 保留为 f64，当前毫秒时间戳与实际流量验证通过；整数域约束需在 T-1-01 通过有依据的 schema/type override 确定。部分事件为 VariantN，不能把编号作为稳定业务 API。完整频道常量生成、语义命名、全部 prototype DTO 替换及 reducer/turn-utils 移植仍属于 T-1-01 等正式实现任务；本轮不提前做无约束的迁移。

阶段 0 尚未整体验收：帧时间及 50 token/s 性能/故障脚本、完整组件样板和阶段报告仍待完成；平台相关签名、公证、打包、干净机按用户要求最后验证。
