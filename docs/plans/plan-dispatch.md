# T-2-05 PlanCard 与待执行计划调度器

基线 PR #35 合并 e8b0f48。依据 01·U4/F-24、03·core::plan_dispatch/PlanCard、04·计划卡、06·T-2-05；用户已授权继续开发、提交和推送。

- 纯 core 承担批准消息格式、持久 pending 的就绪判断和每会话、连接代际内的单飞/合并唤醒策略。匹配固定后端原子 claim===true，不从事件文本、计时或客户端 markCompactionComplete 推断就绪。
- 新原生 PlanCard Entity 用现有 Kit Markdown/TextView、Button/Theme 显示计划，仅最后一条计划提供操作；接受后显示已接受。固定后端把计划路径存入 plan message.content，优先 planPath，兼容读取该规范路径；新增类型化只读 file:read 取得 Markdown，不直接从 UI/本机读取文件。
- 接受时捕获当前会话草稿，safe 先 ACK 切 allow-all，ask 保持用户选择，再发送批准消息；新编辑不被清除。接受并压缩先 ACK 持久保存 planPath/draftInputSnapshot，再发送 /compact；保存失败不压缩。
- 打开/恢复会话、compaction_complete 和会话空闲唤醒同一调度器。忙/awaiting/已 dispatched 不派发；claim 后 detached worker 完成会话绑定发送，不因窗口销毁/切换取消；拒绝或不确定发送不自动重试、不清除已认领记录。成功才 clear，保留固定后端 at-most-once 语义。
- 原生视图仅发语义 intent；app/client 负责 RPC。SessionStore 保存显示/操作临时状态，既有 SharedAppState 持有跨原生窗口的共享调度器，不新增 static。
- core/RPC/原生组件回归、真实固定后端压缩及重启恢复契约、两项独立审查、fmt/clippy/i18n/codegen、三平台 CI 后交付 PR。沿用 DESIGN.md 与上传视觉原型的计划卡层级，不更改会话布局。

阶段0/P0、物理桌面、参考机与 Developer ID/公证继续开放。下一项 T-2-06 附件流程。

真实重启回归发现后端 cold metadata 丢失内存 pending 状态，追加 P-06（消息懒加载恢复字段/claim 前确保加载），不修改子模块固定提交与 wire；冷加载和并行 claim 回归由 backend build 强制执行。
