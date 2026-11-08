# T-2-01 headless 代理与防休眠偏好补丁

基线：PR #31 合并提交 85fc3a4；后端固定 v0.2.6，子模块不变。

- [x] P-01：headless 专用代理保存处理器返回 restartRequired；启动前应用已保存代理。复用 Electron 路由语义，覆盖 undici 和实际 Bun fetch，HTTP/HTTPS、NO_PROXY、禁用/重配和不泄漏代理凭据。
- [x] P-02：headless 专用 keep-awake 保存处理器，严格参数校验，不引入 Electron 或实际平台电源调用。
- [x] Rust 类型化保存回执与真实 sidecar 设置→旧进程保持→重启→读回/请求路由验证，三平台 CI。
- [x] 后端补丁测试与上游 validate:ci；若固定上游已有失败，记录基线和归因，不伪称全绿。
- [x] 独立规格/规范审查通过；上游贡献补丁和 PR 文案已准备。
- [ ] commit/push/PR 与最新 head 三平台 CI。
- [ ] 上游贡献发布：第三方仓库发帖等待用户授权。

设置界面、客户端实际防休眠分别归 T-2-02/T-2-19；不关闭阶段0/P0与正式签名门槛。
