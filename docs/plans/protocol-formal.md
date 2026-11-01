# T-1-01 正式协议生成与 T-0-11 风险报告

基线 main ad5083a。依据规格 02、05、06、07。用户要求继续开发，真实 Developer ID / 公证凭据暂缺，保留 PR #6 工程包。

1. 正式生成：显式根类型清单和后端提交锁；拒绝脏源码/未同步 gitlink；从 TS AST 提取频道、协议版本和消息大小常量，不执行上游代码；整数 override 有精确旧值断言；SessionEvent 使用语义名称；有效 schema / 来源摘要 / Rust 全部纳入只读 drift 检查。
2. 消费：当前 API 改用生成频道常量；对话入口复用 DecodedSessionEvent，已知损坏载荷必须拒绝，未来事件无损保留。完整 API 门面和 reducer 迁移留在 T-1-03/T-1-06，prototype 仅作临时 UI 投影。
3. 风险报告：汇总阶段 0 证据、未通过项、工作量范围；不把凭据暂缓解释为验收通过，不合并 PR #6。
4. 验证：Node 先失败再通过（锁定/override 漂移/静态提取/命名稳定）；Rust 整数和入口拒绝回归；生成漂移、fmt、clippy、核心与真实后端契约；PR 三平台 CI。完成后一次独立全分支审查，修复重要问题。

接口检查：生成清单供 xtask 消费，schema 命名供 DecodedSessionEvent 使用；Rust 现有 reducer 不依赖 VariantN，改名不影响业务分支。
