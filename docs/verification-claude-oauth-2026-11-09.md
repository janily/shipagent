# T-2-04 Claude Pro/Max OAuth

基线 PR #34 合并提交 `19777c0`；范围为 01·F-06、03·oauth_claude 与 06·T-2-04，[本轮计划](plans/claude-oauth.md)。后端继续固定 v0.2.6，P-01–P-05、协议频道和生成根均未改变。

## 行为与边界

设置和首次引导共用 Kit Sheet 中的 masked Input。后端 start 生成 PKCE，原生校验 Claude HTTPS 授权地址、唯一 state/challenge/method/redirect_uri，worker 打开浏览器后才允许提交。用户粘贴授权码，按钮或 Kit Input 的 Enter 提交后立即清空编辑与撤销历史，防止重复提交；等待码十分钟超时，取消在交换期间也可用。错误只显示双语固定提示，不直接展示提供商响应或秘密。

使用独立 `claude-max-数字` slug。token 仅在 SecretString 中短暂消费到 setup，附带可选 identity；保存后调用真实连接探测，再确认目录认证状态，首次引导另读回默认连接。只有全部确认且前台仍拥有该流程才提交成功。API 位于 client facade，app 负责调度，capabilities 只负责 OS 浏览器。

固定后端 Claude PKCE 状态是单例且没有跨客户端 flow 所有权参数；现有 SharedAppState 持有共享互斥锁，原生窗口之间不排队覆盖。锁覆盖 start、等待、交换、clear；本轮没有声称独立外部客户端并行安全，也没有引入新的 static。取消、关闭、超时、错误和断线 best-effort 清除后端 state/新连接/新凭据，不覆盖已有凭据。清理期间保留已关闭取消 sender 标记，确保取消后断线仍能释放引导 busy。

成功结果经 PendingConnection 守卫交接到前台；代际失效、取消、窗口或接收通道销毁均清理独立新 slug，仅当前前台确认后解除清理责任。守卫丢弃不会清除可能属于后续登录的 PKCE。v0.2.6 `chatgpt:logout` handler 实际按 slug 删除任意 LLM 凭据，因此 facade 用它补齐“交换已保存凭据但 setup 未成功”的孤儿清理；有明确注释，未更改协议。

## 本地验证

- 核心 `cargo test --locked` 281 项通过；应用 41 项通过，5 项资源测试默认忽略；引导 5 项、设置 12 项通过。共享 mock support 仅声明一次。
- 模拟真实 WebSocket 验证六个顺序场景：正常交换/identity/setup/连接探测/默认值/目录、错误码、setup 失败期间取消、窗口关闭、重复 redirect_uri 拒绝、成功结果未接收时丢弃清理。另验证第二窗口不发送 RPC 不清理第一窗口，以及等待码超时/关闭。
- 固定真实后端契约：PKCE S256/固定 redirect_uri，clear 后 exchange 被拒、连接数未变化，正常 shutdown；不访问提供商 token endpoint、不使用账号或凭据。
- 显式真实后端 recovery 契约通过，新增“取消→清理待回执→断线恢复”回归，确认取消标志与引导 busy 都释放。原生组件覆盖未 ready 禁止提交、空码无请求、trim/单次提交、交换中取消以及双入口 intent。
- fmt、全 workspace/all-targets/all-features clippy、376 个双语键与 35 根 codegen 检查通过。规格与规范两项独立审查复查通过；取消标志和成功结果所有权交接问题在审查后修复并补回归。

复用上轮固定后端构建和工具缓存；本地 ripgrep 官方下载限制与源码缓存来源见 [T-2-02](verification-extended-settings-2026-11-08.md)。当前 head 的三平台 CI、正式下载和完整原生 E2E 结果见 PR。

## 原生视觉

实际运行 Linux Xvfb/lavapipe 与 Kit Settings 示例，明确展示夹具打开同一个 ClaudeLoginForm Sheet，没有登录账号或交换 token。已检查亮暗布局及取消入口：

- [亮色](images/claude-oauth-light.png)
- [暗色](images/claude-oauth-dark.png)

## 保留门槛

真实订阅账号登录、浏览器默认关联与系统焦点、物理 IME/辅助功能需人工验收；模拟成功交换不代替 F-06 真人验收。阶段0/P0、实体桌面与参考机性能仍开放，Developer ID 签名/公证按用户约定暂缓。下一项 T-2-05 PlanCard + 待执行计划调度器。
