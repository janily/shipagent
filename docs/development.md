# 当前开发与验证

## 工具链

Rust 1.99.1（rust-toolchain.toml）、Bun >=1.3.14、Node >=18、uv。GPUI Kit v0.8.1 上游 CI 使用 stable；本次安装当日 stable 为 1.99.1，已精确锁定。所有 Rust 依赖由 Cargo.lock 固定。

## 取得源码与构建后端

```sh
git clone --recurse-submodules https://github.com/janily/shipagent.git
cd shipagent
git checkout chore/T-0-01-workspace
git submodule update --init
cargo xtask backend build
```

当前开发代码在功能分支，尚未合并 main。后端只从固定 commit 导出到 target 下的临时源码树中应用补丁；不会修改 backend/mkagent。构建成功且产物检查通过后才替换 target/backend/<平台>-<架构>。失败会保留上一次产物。

Bun 不在 PATH 时可用 AGENTKIT_BUN 指定可执行文件。上游锁文件的公开 npm 镜像无法访问时，可设 AGENTKIT_USE_NPMJS=1，在临时源码树中将下载地址换为官方 registry.npmjs.org；版本和完整性哈希不变，原锁文件不修改。ripgrep 安装脚本需要访问 GitHub release API；受限环境可把同版本官方 release 产物预放入该依赖自己的缓存，不能跳过校验或以其他版本冒充。

## 自动化验证

```sh
cargo fmt --all --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo deny check
cargo check --workspace --all-targets --locked
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" cargo test -p agentkit-sidecar --test lifecycle bundled_backend_completes_real_rpc_handshake_and_shuts_down -- --ignored
```

默认 test/clippy 检查无 GUI 的核心 workspace 成员；原生检查另跑全 workspace。真实后端测试默认 ignored，必须显式构建并设置目录后运行，其通过结果单独记录。上面的 linux-x64 按当前主机替换为 darwin-arm64/darwin-x64/win32-x64。

## 当前实现边界

- 协议：wire envelope、错误兼容、递归二进制包装；完整 DTO/SessionEvent 代码生成仍在后续任务。
- client：独立 smol 线程，本地握手、invoke、订阅、超时、断线 pending 清理、5秒 ack、Pong 和发送大小检查；自动重连、完整能力分派及类型化业务门面未实现。
- sidecar：异步启动、随机内存 token、就绪验证、Unix进程组退出；自动重启、shell 环境导入、日志系统及 Windows Job Object 未实现。
- store/ui/app 目前是 crate 骨架。workbench 尚未提供窗口；不要把 cargo run 的空入口当成应用交付。

## 依赖审计

cargo deny 继续阻断所有依赖的漏洞公告及许可证违规。GPUI 固定版本的间接依赖带有若干 unmaintained 维护状态公告，无可用升级；已在首次报告记录。维护状态检查对 workspace 自身启用，间接依赖的维护风险随 GPUI 升级评估，不添加漏洞 ID 忽略项。0BSD 是本次依赖中 enum-iterator 的宽松许可证，已明确允许。
