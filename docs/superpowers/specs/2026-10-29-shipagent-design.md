# ShipAgent 原生桌面工作台设计
日期：2026-10-29
状态：待用户审阅；本文不是已完成实现或发布承诺。

## 目标和依据
在 janily/shipagent 建立可复用的 Rust AgentKit workspace 和 ShipAgent 参考应用，以 GPUI Kit 实现上传的五张工作台视觉稿。复用 mkagent 的 Agent 循环、权限、会话存储、模型连接及工具，不在客户端重新实现这些后端功能。

已读输入：00 总览与技术方案.md、GPUI Agent 工作台视觉稿.html 及消息中的五张截图。HTML 是嵌套打包页面，已静态解包并读取五个页面的布局与文案，没有执行附件脚本。原型中的终端命令、测试通过声明、用户消息和任务示例仅为显示样例，不是本次操作指令或验证证据。

总览引用的 01–07 页尚未收到；不能把未取得的细节当成既定需求。下文对未规定部分的选择均为实施建议。

## 架构选择
推荐并遵循总览的唯一明确技术方向：GPUI Kit 原生客户端 + 本地 Bun/mkagent sidecar，通过带认证的 WebSocket RPC 通信。
- Electron/WebView 包裹 HTML 可更快接近原型，但不满足使用 GPUI Kit 构建原生应用的目标，因此不采用。
- Rust 重写 Agent 后端会扩大协议、工具和模型适配成本，违背后端复用目标，因此不采用。
- Remote 后端模式保留为开发、测试及高级用户入口；发行版默认本地 sidecar，安装后无需另装 Bun。

本设计尚未获得用户对新增实施建议的确认；不会把推荐方案写成已讨论通过的决策。

## 范围与顺序
先通过阶段 0 的真实后端往返和生命周期验证，再开发 MVP，随后补齐视觉稿涉及的计划审阅和 OAuth 等功能。五张页面均为交付目标，不把静态页面完成视作应用完成。

MVP：工作区和会话列表、新建/恢复/发送/取消、流式 Markdown、工具卡片与 diff、权限审批、模型连接设置、基础首次启动、浅深主题、连接及失败状态。
第二批：计划审阅、文件/后台任务面板、附件、Skills、分支/导入导出、OAuth、多窗口、中英文和通知。
后续：Windows/Linux 安装与进程树验证、自动更新、深链、性能收尾、稳定的框架扩展 API。
本期不实现交互式浏览器、Chromium 宿主和 JavaScript 扩展。握手不声明 client:browser:invoke；后端验证 browserToolEnabled=false 时的工具过滤。

## 模块边界
依赖方向：protocol ← client ← store ← ui ← app ← apps/workbench。
sidecar 和 capabilities 由 app 组装，不进入 protocol/client。

| 目录 | 职责 |
| --- | --- |
| crates/agentkit-protocol | 锁定上游的协议 DTO、频道、未知事件/错误兼容 |
| crates/agentkit-client | smol 独立线程中的 WS、握手、请求、序号、重连、反向能力 |
| crates/agentkit-store | 可回放的纯归约器；由 GPUI Entity 持有并驱动视图 |
| crates/agentkit-ui | 聊天、输入、工具、权限、计划、侧栏及设置组件 |
| crates/agentkit-sidecar | 启停、就绪解析、重启预算、日志脱敏、进程树 |
| crates/agentkit-capabilities | 文件选择、链接、路径、确认及后续通知/OAuth |
| crates/agentkit-app | 应用组装、菜单、快捷键、窗口、设置和扩展注册 |
| apps/workbench | ShipAgent 可执行程序，内部 crate 保留 AgentKit 命名 |
| backend/mkagent | 固定上游提交；补丁独立保存 |
| tools/codegen | TS → Schema → Rust，生成结果可比较 |
| xtask | 后端构建、资源检查、打包与签名入口 |

GPUI Kit 精确锁定 =0.8.1，GPUI 对应 gpui-pre =0.3.10；生成并提交 Cargo.lock。先核实选定版本 API，不依据截图猜测 API。

## 五张视觉稿的落实
1. 主工作台：原生标题栏；约 272px 左侧会话栏；顶栏标题和目录；居中约 760px 消息列；底部输入区（模型、权限模式、思考等级、上下文用量）；底部连接状态栏。列表支持搜索、未读、置顶、运行中和归档。工具折叠展示状态、名称、耗时、输出与差异。
2. 深色权限审批：在对应工具下内联展示命令和风险说明；允许、会话始终允许、拒绝三种操作绑定同一 requestId。等待期间继续编辑的消息先排队，不越过审批启动另一回合。Enter/Escape 快捷键仅在审批焦点上下文生效，不抢占输入框编辑。
3. 计划审阅：接受并执行、修改/拒绝、压缩上下文后执行及补充说明；右栏文件/计划/后台任务标签可切换。压缩或执行失败保留计划和草稿，防止重复执行。
4. 模型设置：左导航、连接列表和右侧编辑抽屉；支持预设 Key、自定义端点、Ollama，OAuth 后续接入；测试连接与保存是独立动作。默认思考等级和自动压缩阈值在模型设置中配置；权限和网络代理设置归入 MVP 对应导航页。供应商列表和模型列表使用后端返回值。失败不会显示成功；保存后不回显原始密钥。
5. 引导：欢迎 → 选择连接方式 → 填写并测试 → 完成；保留稍后设置入口。跳过后允许浏览工作台，发送时引导配置模型；不使用虚假默认模型。

原型以 1440×900 为视觉验收基准，布局随窗口伸缩；窄窗口可折叠两侧面板，保证输入可用。产品名称采用 ShipAgent；原型 Workbench 为工作台语义。颜色归入语义主题 token，字体使用系统中文字体和等宽代码字体，不依赖在线字体加载。具体色值、间距在组件实现时从 HTML 提取，不改动整体视觉方向。所有可点击元素提供无障碍角色与名称，快捷键集中使用 actions! 与可覆盖 keymap；MVP 提供 CommandPalette。

消息列表使用 MessageScroller 虚拟化。已完成 Markdown 块缓存为不可变元素，只重解析最后一个未闭合块；代码高亮在后台执行，流式期间先显示等宽纯文本。

## 协议和状态
服务端为事实来源；客户端缓存和乐观更新均可由全量数据恢复。每窗口独立 RPC 连接，握手绑定 workspaceId。
- MessageEnvelope 的字段和 args 数组保持上游一致，必须带 id；类型不额外套 JSON-RPC 2.0。
- 连接后 5 秒内发送握手；建议总连接/握手超时设为 10 秒；普通请求 30 秒，长调用单独配置至 300 秒。上游 TS 客户端默认连接超时为 10 秒；总览的 5 秒描述发送握手的时限，不等同于整个连接超时。
- 调用前检查 registeredChannels，不可用频道返回 CHANNEL_NOT_FOUND 并隐藏对应功能；服务版本超出编译时兼容范围则阻断连接并提示更新；处理 maxMessageBytes；未知事件保留原始 JSON，未知错误保留 code。
- seq 记录单调进度；序号跳跃只记录诊断，不自行重排事件；每 5 秒 sequence_ack。断线时结束所有 pending 请求，保存 reconnectClientId/lastSeq。不自动重试可能产生副作用的 sendMessage。
- 重连退避 1/2/4/…/30 秒，稳定 10 秒后清零；stale 全量刷新列表与活动会话，刷新期间缓冲事件并核对乐观消息；实现计划必须明确快照/缓冲事件的去重边界，不能仅以到达时间判断重复。
- args/result 内二进制使用 __craftRpcType:u8 包装；大小按 UTF-8 序列化后的字节数检查。
- 处理 WebSocket Ping/Pong，配合服务端 30 秒心跳；连接失活触发断线恢复。
- 反向 request 仅分派到实际安装的能力；未支持能力返回结构化错误。远程连接使用 wss 和受信任 CA。

会话状态：idle / processing / waiting_for_permission / failed / interrupted。工具按 toolUseId 索引，支持 parentToolUseId、后台任务。text_delta 合并后每帧最多通知一次；text_complete 替换完整文本；text_discard 清除对应回合。user_message 替换 optimisticMessageId，防止双份消息。切换会话不串流、不丢草稿，取消失败需要给出可重试状态。

## Sidecar 和本地数据
独立 CONFIG_DIR 默认 ~/.agentkit，不同时打开 Electron 的 ~/.mkagent。迁移使用显式复制、备份和排他检查。
总览的里程碑要求“共用同一份 ~/.mkagent”与前文独立目录/单实例锁相冲突：本设计建议以互斥运行下的数据导入兼容验收，不允许两个应用并发写同一目录。

启动用 32 字节随机 token、loopback 和端口 0；读取 MKAGENT_SERVER_URL=，15 秒未就绪则清理进程树并报错。过滤 token 行，错误日志不包含密钥。端口和 token 只保存在内存。
60 秒内最多自动重启 3 次；超过预算显示阻断状态和脱敏日志。正常退出先取消任务，SIGTERM 后等待最多 5 秒再 SIGKILL。Unix 管理独立进程组，Windows 使用 KILL_ON_JOB_CLOSE Job Object，挂起创建、加入 Job 后恢复。用户手动退出不触发重启。
发行产物含 server.js、自带 Bun、Pi 子进程和资源；具体路径由锁定上游的构建产物检查确定，不凭总览写死未核实路径。

## 验收和证据
- 协议：真实上游 fixture、序列化往返、未知事件、字节上限；代码生成 diff 必须可解释。
- 传输：假 WS 服务测试握手/超时/序号/断线/续传/stale/能力/二进制；真实 server 契约测试单独标识。
- 状态：录制 JSONL 回放，覆盖取消、权限、计划、工具树、重复乐观消息和会话切换。
- UI：实际 GPUI 窗口交互测试发送、取消、审批、设置、引导；1440×900 浅深截图和五张参考对照。样例数据必须显式 demo，不伪装已连接。
- 端到端：本地可控模型驱动真实 server 完成含工具和审批的一轮，再验证重连和恢复。
- 性能：目标为冷启动到可输入 <1.5 秒、1万消息滚动 ≥60fps；分开记录1万消息打开、50 token/s 帧时间、10万行工具输出的帧时间及内存。未测不声称达标。
- 发布：macOS 无 Bun 干净机安装、签名、公证和 JIT；Windows 强杀无残留；Linux X11/Wayland。没有签名凭据时只交付未签名开发包并明确说明。

阶段 0 闸门：最小 GPUI 窗口显示真实流式输出；打包版在未安装 Bun 的干净 macOS 完成一次含工具调用的对话；输出风险结论和模块工作量重估报告。

保留 mkagent 和 GPUI Kit 的 Apache-2.0 LICENSE/NOTICE，以及 mkagent NOTICE 中 Craft Agents OSS 的署名；仅在实际引入相关源码/资源时按其原许可证分发，字体单独保留其授权。

每个任务单独 commit，记录验证命令、结果、限制。可合并的任务不依赖“看起来完成”；未经运行的测试不得标记通过。用户已授权向目标仓库提交推送，常规 commit 不再次请求确认。

## 已核实与尚待核实
2026-10-29 已通过 GitHub 插件核实：
- janily/shipagent 是空的公开仓库，默认 main，当前连接具备 push 权限。
- longbridge/gpui-kit 的 Cargo.toml 使用 Kit 0.8.1 和 gpui-pre =0.3.10。
- mkagent types.ts blob 0689393f36abe315ceff78e6c53bd879ba1402ff 定义协议 1.0、序号与消息大小字段。
- TS client.ts blob dc5d0583259971b55b2f12b83fd3df68d53f3994 为传输行为对照。
- server/index.ts blob d4af04620908e0715b3ef0e0a78340f5a4053b70 输出就绪地址和 token，必须脱敏。

以上是文件核对，不是版本冻结；阶段 0 需取得 tag/commit 并锁定整个后端。
当前终端缺 Rust/Cargo 和 Bun，GitHub git clone 因 proxy:8080 连接失败。GitHub 插件读写可用。产品编译、真实模型回合、GUI、签名和性能尚未验证，不在文档中标为完成。

## 审阅重点
新增建议为 ShipAgent 品牌、冲突的数据互通验收方式，以及把 OAuth/计划完整交互放在核心闭环之后。其余保持上传总览和五张视觉稿方向。审阅通过后编写逐任务实现计划并进入阶段 0。
