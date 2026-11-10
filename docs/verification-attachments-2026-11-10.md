# T-2-06 附件流程验证

基线 PR #36 合并 `ca60d93`，[计划](plans/attachments.md)。上游仍固定 v0.2.6 / P-01–P-06，本轮未改后端或扩大 wire。

## 行为

多选文件、图片/文件粘贴、会话区域拖入共用背景读取；选择器返回绑定发起会话，普通文本粘贴与 IME 保持。剪贴板字节不可变共享，避免 intent 克隆大载荷。

普通文件读取前后校验；50 MiB 本地上限、有界 read 防增长文件；Unix 非阻塞打开防 FIFO/替换竞态挂起。图片解码限制内存/尺寸，生成最长边 ≤256 px PNG 缩略图，损坏或不支持图片逐项提示。Kit Attachment 预览与单项移除；上传准备、转换复制和 JSON 编码/发送全在后台。读取中禁止发送，附件非空允许空文本发送。

逐个 storeAttachment，失败项显示本地 warning 并保留；成功项带服务器 storedPath/markdownPath、mimeType、resizedBase64 发送。全失败无文本不发空消息；ACK 仅在捕获连接代际仍有效时消费成功 ID，文本按捕获 revision 清理；旧结果只结束自己的乐观标记，不清理恢复后的草稿。瞬态缩放字节不进入长期 StoredAttachment 展示状态。同进程切换、后端恢复缓存重建保留附件，跨应用重启自动持久草稿归 T-2-08。

## 本地证据

- 核心基线 285 项，新增附件 ACK/部分失败/新编辑归属回归后 286 项通过。
- 应用 46 项通过，7 项资源测试默认忽略；包含受限读取、文本/二进制/损坏图、PNG 缩略图边长、FIFO 拒绝与真实 WebSocket 精确存储/发送参数。
- 模拟握手 4096 字节上限，过限附件本地拒绝且没有该存储请求；其余成功项继续发送，不扩大门禁。
- 原生 composer 12 项通过：附件单独发送、读取 gate、普通文本与图片粘贴、会话区拖入、多选返回/跨会话归属、实际 hover/remove 点击。
- 显式真实固定后端：文本附件和空文件，后者 store 失败，成功项以空文本发送；核对落盘内容、用户消息附件，以及重启后引用与内容。使用本地脚本模型，无提供商账户，加入三平台 native-check。
- 上轮计划契约首个快照仅含 Assistant 时会 unwrap；本轮有界等待真实 plan，超时保留消息诊断，不把任意 Assistant 当作 SubmitPlan 已可见。本地压缩→重启重跑通过，Windows 仍以当前 head CI 为准。
- fmt、397 双语键、35 根 codegen、全 workspace/all-targets/all-features clippy 通过；规格/规范两轴及修复增量复核完成。完整原生 UI/app/capabilities 套件另有 179 项通过；三平台最终状态见 PR 当前 head。

恢复后临时缓存链接失效，清理可再生 debug 缓存重新编译；完整原生套件链接耗尽 8GB overlay，迁移编译缓存、清理可再生示例二进制后重跑。默认沙箱 socket/Node 子进程受限检查在同环境网络权限下重跑，最终只引用成功结果；未改依赖或门禁绕过验证。

## 原生视觉

Linux Xvfb/lavapipe 实际渲染 `examples/attachments`，完整原生 WorkbenchView 使用明确的本地种子夹具，不代表截图中执行模型回合或完成物理验收。

- [亮色](images/attachments-light.png)
- [暗色](images/attachments-dark.png)

## 保留容量验收

**F-26 完整 50 MB 容量尚未通过。** 固定 Bun `maxMessageBytes=16 MiB`，单文件 base64 或多文件聚合可能超限。50 MiB 是本地读取上限，不承诺所有 ≤50 MB 均可发送；超限报错、保留草稿，不扩大协商限制或静默删载荷。完整容量需要另外设计上传协议/后端能力，逐项警告不能替代容量验收。

下一项 T-2-07 @ 提及、/ Skill；T-2-08 草稿持久化。阶段0/P0、实体 IME/辅助功能、参考机性能、Developer ID/公证仍开放。

首轮 CI i18n 发现示例模型名称被源代码扫描识别为界面文案；移入明确的样例 JSON 后按同一门禁验证。
