# Phase Zero Implementation Plan

> For agentic workers: use superpowers:executing-plans to implement tasks in this session.

**Goal:** 建立可验证的原生 workspace、固定后端构建、RPC 与 sidecar 原型，并逐步完成阶段 0 闭环。
**Architecture:** 按 03 页 crate 边界；不在 UI 中实现后端业务；纯协议/传输可单独测试。
**Tech Stack:** Rust 1.99.1（上游 v0.8.1 CI 使用 stable，当前 stable 1.99.1），edition 2024；Kit =0.8.1；smol 2；Bun >=1.3.14。
**Spec:** docs/spec/00–07；任务编号保持第 06 页。

## Global Constraints
- 后端固定 6e8ebc456b6b82441be09118ca11afca79b84836；补丁不提交到子模块。
- 所有 Rust 依赖在 workspace.dependencies；不直接依赖 tokio。
- 无产品代码时不宣称原生应用已可运行；构建、测试、GUI、签名证据分别记录。
- 品牌集中 apps/workbench/brand.toml；默认 ~/.agentkit。

## Review Focus
- 后端构建中途失败不得遗留应用中的半成品。
- 重复构建不得再次套补丁而失败，也不能覆盖开发者的子模块修改。
- token 不进入 Debug/日志；就绪 URL 只接受本地合法动态端口。
- 断线结束 pending；请求不在握手前发送；写请求不自动重试。
- 未知错误/事件和 UTF-8 字节限制必须按 wire contract 处理。

### T-0-01: Workspace
Files: Cargo.toml、Cargo.lock、rust-toolchain.toml、.cargo/config.toml、.github/CODEOWNERS、.github/workflows/ci.yml、crates/*/Cargo.toml 与 src/lib.rs、apps/workbench、xtask。
Produces: 可独立检查 protocol/client/sidecar/store core 的 workspace；GUI 依赖使用原生分层。
- [ ] 建立指定 crate、集中依赖、锁定工具链、品牌和配置。
- [ ] cargo metadata / fmt / check / clippy，生成 Cargo.lock。
- [ ] 配置 lint/unit 与审计，CODEOWNERS 不误称可强制两人审批（GitHub 分支规则另行配置）。
- [ ] Conventional Commit 引用 T-0-01。

### T-0-02: Backend
Files: .gitmodules、backend/mkagent、backend/patches/P-03-no-webui.patch、xtask/src/backend.rs。
Produces: cargo xtask backend build，校验固定 commit、隔离 staging、失败不替换可用产物。
- [ ] 从真实脚本生成 P-03，验证 git apply --check 和正常 WebUI 路径未改变。
- [ ] 先测试未知命令、缺 Bun、产物检查和重复构建条件，观察失败。
- [ ] 编写构建调度，使用隔离 checkout 应用补丁，执行 bun install 与 build-server --no-webui。
- [ ] 测试通过后再实际构建；失败保存日志并记录，不能标记任务完成。
- [ ] 单独 commit 与推送。

### T-0-04: Protocol and RPC prototype
Files: protocol/src/{envelope,error,codec}.rs、client/src/{lib,transport}.rs、各自 tests/。
Interfaces: RpcClient::connect(ClientOptions)、invoke(channel,args)、subscribe(channel)、close；typed API 后续 T-1-03 完整实现。
- [ ] 先写 wire fixture 测试，验证 camelCase、allow-all、二进制包装和未知错误。
- [ ] 实现最小 envelope/codec，并以真实协议固定值验证。
- [ ] 建立本地 WS 服务测试握手后 invoke/event、超时、断线和能力错误。
- [ ] 实现独立 smol 线程连接和请求分派；仅原型范围，完整重连归 T-1-02。
- [ ] cargo test / clippy，真实后端契约通过才记录完成。

### T-0-03: Sidecar prototype
Files: sidecar/src/{lib,readiness}.rs、tests/。
Interfaces: SidecarConfig / SidecarHandle::start / endpoint / shutdown，Endpoint 包含 SecretString。
- [ ] 先测试合法就绪 URL、非本地地址/端口0拒绝、token 行过滤、超时与提前退出。
- [ ] 实现后台进程启动、就绪检测与退出，不在 GUI 前台阻塞。
- [ ] 测试与实际后端运行分开记录，重启/跨平台完整进程树归 T-1-05/T-3-01。
- [ ] 单独提交。

### 后续阶段 0 任务
T-0-05/T-0-06 原生流式窗口与真实回合；T-0-07 codegen 可行性；T-0-08 fake-llm；T-0-09 macOS 签名/干净机；T-0-10 组件样板；T-0-11 实测报告。继续逐任务细化，阶段门槛按第 06 页不降低。
