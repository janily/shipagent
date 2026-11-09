# T-2-03 ChatGPT Plus OAuth

基线为 PR #33 合并提交 `5cc452c`。实现 06·T-2-03、01·F-05 与 03·oauth_chatgpt；[计划](plans/chatgpt-oauth.md)。后端仍固定 v0.2.6，补丁 P-01–P-05、协议频道及生成模型不变。

## 行为

原生设置和首次引导共享登录控制器：先绑定 `127.0.0.1:1455`，再启动后端 OAuth，校验官方 HTTPS 授权 URL、唯一 state 与固定 redirect_uri 后打开浏览器。监听只接受指定 Host、GET、回调路径及匹配 state，限制请求大小、单连接时间和五分钟总时限；非法请求不消耗合法 flow。回调页面仅显示已收到、可关闭的双语提示，不回显 code/state。使用现有 async-net/smol，实现与规格等价的异步本地 HTTP，不新增 tiny_http。

PKCE 与 token 交换仍归后端；原生端不读取 token，不记录授权 URL、code 或 state。成功经 complete、创建唯一新连接、认证状态与目录读回确认；首次引导另设置默认连接并读回。每次登录使用独立 slug，避免覆盖已有凭据。取消、关闭窗口、断线和重启关闭监听并 best-effort 取消指定 flow；交换已开始则等待其结束，清理本次新连接和凭据。即便后续读取目录失败，取消仍执行清理。重连释放引导 busy，允许重新操作。真实浏览器返回时请求激活应用窗口。

业务 RPC 留在 app 控制器与类型化 client facade；capabilities 只负责浏览器及回调 I/O。Claude 入口仍按下一项任务处理。

## 本地验证

- 核心 Rust 281 项通过；应用单测 39 项通过、5 项资源测试默认忽略；原生引导 4 项、设置 10 项通过。
- capabilities 9 项通过，包含真实本地 HTTP、非法路径/state、重复参数、拒绝、取消、超时及端口释放。应用 OAuth 模拟 RPC 测试使用真实 WebSocket 和本地 HTTP：完整回调与交换参数、setup 后失败并取消清理、重复 redirect_uri 拒绝三个场景通过；上游交换成功响应为明确夹具。
- 固定真实后端 `chatgpt_oauth` 契约通过：生成 PKCE S256 授权 URL、跨客户端 flow 所有权隔离、取消后完成被拒且无连接产生、正常 shutdown。测试没有访问 OpenAI token endpoint，也没有使用真实账号。
- 显式运行真实后端控制器 recovery 契约通过；新增断线取消及引导 busy 释放回归。常规测试的 ignored 不代替这些显式结果。
- fmt、全 workspace/全目标/全特性 clippy、369 个双语键集及 35 根 codegen 检查通过；规格和工程规范两项独立审查复查通过。

本地沿用 T-2-02 工具缓存和固定后端构建。官方 ripgrep 下载的本地 403/504 替代方式见 [上一轮报告](verification-extended-settings-2026-11-08.md)；本轮没有更改锁定版本、下载配置或补丁。三平台 CI 验证正式下载路径，最终以 PR 最新 head 为准。

Linux 首轮 CI 在上游文档工具验证报告 Python 缺少 `_sysconfigdata_`。核查 rust-cache 的递归清理实现与 xtask 路径后，发现验证解释器/uv 缓存位于会被裁剪的 target 目录；改为每次构建独占的 scratch 子目录，随构建清理，避免恢复残缺安装。没有跳过上游验证或改变 Python 需求；后续当前 head CI 重新验证。

## 原生截图

Linux Xvfb/lavapipe 实际运行 Kit SettingsView 示例，以明确的 OAuth 等待态展示夹具捕获；不含账号或凭据，也不代表真人登录完成。已检查亮暗布局与取消入口：

- [亮色等待态](images/chatgpt-oauth-light.png)
- [暗色等待态](images/chatgpt-oauth-dark.png)

## 保留门槛

最新 head 的 Linux/macOS/Windows CI 与 macOS 工程包结果见 PR。真实订阅账号的浏览器登录、OS 默认浏览器往返/窗口激活、实体桌面、IME/辅助功能仍需人工验收；自动化不能据此签署 F-05 真人登录。阶段0/P0、参考机性能及 Developer ID 签名/公证继续开放。下一项 T-2-04 Claude OAuth。
