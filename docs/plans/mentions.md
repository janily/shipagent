# T-2-07 @ 提及与 / Skill

基线 PR #37 合并 0b7a324。执行 01·F-27/U5、02·提及语法、03·core::mentions/MentionMenu 与 04·输入区。

1. 移植固定上游的 bracket 解析/生成/badge 语义；ASCII skill 语法、工作区与 .agents 来源限定、去重、UTF-16 位置。纯核心对照上游用例。
2. Kit Textarea 原子 inline token 显示 Tag，保留输入法、撤销、原子删除；MentionMenu Entity 使用 Popover/List，输入保持焦点。@ 文件/文件夹分组、150ms 防抖，行首 / Skill/compact，↑↓、Enter/Tab、Esc。
3. 控制器用已绑定连接与真实会话工作目录发 fs:search；会话/连接/请求代际隔离，旧结果不覆盖新 query，失败可重试。skills:changed 使用现有工作区缓存。
4. 普通、附件、重发发送均传正确 options.badges/skillSlugs；乐观与用户消息展示 badge。不执行被提及的 Skill、不解析指令作为本轮权限。
5. 核心/原生/RPC 回归，真实本地模型契约，截图，双轴审查，三平台 CI 后推送 PR。草稿持久化归 T-2-08；既有容量/阶段0/P0/正式签名验收继续开放。

执行记录：核心与 12 组固定上游 oracle 对照、Kit 原生交互、控制器目录/连接隔离及发送元数据均已实现；真实后端契约和本地回归通过。交付证据见 [验证报告](../verification-mentions-2026-11-10.md)，三平台最终状态见 PR。
