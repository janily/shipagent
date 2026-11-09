# T-2-04 Claude Pro/Max OAuth

基线 PR #34，合并提交 19777c0。范围为 01·F-06、03·oauth_claude 与 06·T-2-04；用户已授权继续逐项开发、提交与推送。

1. 类型化 facade 复用 startClaudeOAuth / exchangeClaudeCode / clearClaudeOAuthState 和 setup；token 只用 SecretString 短暂传递到 setup，identity 同步保存，不读取已保存凭据。
2. app 控制器持有窗口级取消及粘贴授权码通道。固定后端 Claude PKCE 状态是全局单例，现有 SharedAppState 持有共享非等待锁，整个 start/交换/清理期间排他；不宣称跨独立客户端隔离。
3. 校验 claude.ai HTTPS 授权地址及固定 redirect_uri/PKCE 参数；worker 打开浏览器。十分钟有效，用户通过 Kit masked Input 粘贴授权码；提交后立即清空，交换失败显示固定错误并允许重新登录。
4. 新建 claude-max-数字连接；setup + 实际保存连接探测 + 目录确认后才显示成功，引导另确认默认连接。取消/退出/断线、失败及部分完成均清理本次新连接与凭据、清除 OAuth 状态，不覆盖旧连接。成功结果由待接收资源守卫持有，前台验代际和取消后才提交；未接收/失效/窗口销毁自动清理新连接。
5. 原生设置与引导共享 Kit Sheet/ClaudeLoginForm；Kit Theme/Input/Button/Sheet 负责视觉、焦点、IME 与交互。controller 负责 RPC，capabilities 仅负责 OS 浏览器。沿用 DESIGN.md / 04，不自建控件体系。
6. 本地模拟 WebSocket 覆盖成功、错误、取消清理；真实固定后端只测准备、清理、过期状态，不访问提供商 token endpoint。原生组件、核心、fmt/clippy/i18n/codegen、独立两轴审查及三平台 CI 后提交 PR。

真实订阅登录、OS 浏览器往返、物理桌面及阶段0/P0/Developer ID 公证保持人工待验。下一项 T-2-05 PlanCard + 待执行计划调度器。
