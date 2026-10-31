# 当前开发与验证

## 工具链

Rust 1.99.1（rust-toolchain.toml）、Bun >=1.3.14、Node >=22（codegen与打包）、uv。GPUI Kit v0.8.1 上游 CI 使用 stable；本次安装当日 stable 为 1.99.1，已精确锁定。所有 Rust 依赖由 Cargo.lock 固定。

## 取得源码与构建后端

```sh
git clone --recurse-submodules https://github.com/janily/shipagent.git
cd shipagent
git submodule update --init
cargo xtask backend build
```

原生流式对话、协议生成与 Linux 性能测量均已合并 main。后端只从固定 commit 导出到 target 下的临时源码树中应用补丁；不会修改 backend/mkagent。构建成功且产物检查通过后才替换 target/backend/<平台>-<架构>。失败会保留上一次产物。

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

- 协议：wire envelope、错误兼容、递归二进制包装；T-0-07 已生成 Message/Session/SessionEvent 并验证真实流量，现与 prototype 边界并行。完整正式生成和迁移仍归 T-1-01。
- client：独立 smol 线程，本地握手、invoke、订阅、超时、断线 pending 清理、5秒 ack、Pong 和发送大小检查；已有阶段0类型化业务门面；自动重连、完整能力分派未实现。
- sidecar：异步启动、随机内存 token、就绪验证、Unix进程组退出；自动重启、shell 环境导入、日志系统及 Windows Job Object 未实现。
- store/ui/app 已提供阶段0原生窗口与完整工具回合：MessageScroller、增量 Markdown、原生 Textarea、发送/停止、显式审批、新建/打开会话、端点配置及断线提示。单活跃回合，运行时禁止切换；完整多会话和其他面板按后续任务执行。

## 依赖审计

cargo deny 继续阻断所有依赖的漏洞公告及许可证违规。GPUI 固定版本的间接依赖带有若干 unmaintained 维护状态公告，无可用升级；已在首次报告记录。维护状态检查对 workspace 自身启用，间接依赖的维护风险随 GPUI 升级评估，不添加漏洞 ID 忽略项。0BSD 是本次依赖中 enum-iterator 的宽松许可证，已明确允许。

## 运行原生窗口

```sh
cargo run -p workbench --locked
```

先构建后端。启动后在窗口填写 OpenAI-compatible Base URL、模型和可选 API Key，保存后发送。默认数据目录由 brand.toml 的 data_dir_name 决定；测试建议用 AGENTKIT_CONFIG_DIR 指向隔离目录，AGENTKIT_WORKING_DIR 指向允许 Agent 操作的项目。AGENTKIT_BACKEND_DIR 可覆盖后端路径。AGENTKIT_LANGUAGE=en 或 zh-CN；主题按钮即时切换。Enter 发送、Shift+Enter 换行，运行时点击停止；macOS Cmd+Q/Linux Ctrl+Q 会先清理后端再退出。

无外部密钥的验证：

```sh
node --test tools/fake-llm/server.test.mjs
cargo test -p agentkit-ui --test workbench --locked
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" cargo test -p agentkit-sidecar --test agent_turn --locked -- --ignored
```

agent_turn 自行启动 loopback 模型服务和隔离后端，实际读取临时文件并验证工具结果及取消。Linux 原生构建需要 GPUI 的 X11/Wayland、xkbcommon、fontconfig 等开发库及可用 Vulkan；CI 已扩展到 Linux、macOS 14 arm64 与 Windows Server 2022 x64 的核心单测、原生编译/测试、固定后端构建与真实 Agent 回合；macOS/Windows 另运行实际窗口冒烟。物理桌面/签名/打包验收见新报告。完整边界见 [本次验证](verification-native-turn-2026-10-29.md)。

## 生成协议并检查漂移

```sh
npm ci --ignore-scripts --prefix tools/codegen
cargo xtask codegen
cargo xtask codegen --check
node --test tools/codegen/schema.test.mjs
```

Node >=22。生成器只读取已初始化的固定子模块类型源，不修改 backend/mkagent；`--check` 在临时目录生成并比较，不覆盖手工修改。详细兼容规则及正式迁移边界见 [生成工具](../tools/codegen/README.md) 和 [本次验证](verification-codegen-2026-10-29.md)。

## Linux 流式性能实验

```sh
cargo build --release -p workbench --features perf --locked
node tools/perf/run.mjs --binary target/release/workbench --backend target/backend/linux-x64 --output target/perf/stream.json --runs 3 --tokens 600 --environment '实际 CPU/GPU/显示环境'
```

需要 Python 3、Linux 显示环境和 Vulkan；Xvfb 无窗口管理器时安装 xdotool 并加 `--focus xdotool`。输出文件须不存在。默认应用不启用 perf。原生工作时间、GPU提交/调度、批次延迟分别报告；不是用 SSE 间隔充当帧耗时。

判定、监督机制、断流/强杀命令见 [工具说明](../tools/perf/README.md)，本次数据和未达标项见 [验证报告](verification-linux-perf-2026-10-29.md)。

## 六类原生组件样板（T-0-10）

```sh
cargo run -p agentkit-ui --features samples --example components --locked
cargo test -p agentkit-ui --features samples --test samples --locked
```

无需后端或 API Key，不访问用户配置。左侧使用 Sidebar；主区使用 Textarea 和 Collapsible；右侧使用 Resizable（主区最小 400px、右栏 200–360px）；设置按钮打开 Kit Settings，主题立即生效，仅限本进程；清空按钮打开 Dialog，取消保留草稿，确认清空样板内容。窗口最小 960×600，中文输入/发送、草稿保留、折叠、确认/取消、空白提交和分栏拖动有原生控件测试。

构建后可运行 `node tools/native-smoke/run.mjs`；Windows 自动选择 `.exe`。需要真实桌面或 Linux X11/Vulkan。程序使用显式 `--smoke-report NEW.json`，在后续帧回调等待最多 10 秒，验证至少一次该窗口的 Present 记录后写报告退出；监督脚本限制 30 秒并核对主机/架构/尺寸，结果写入不存在的 `target/native-smoke.json`。重复运行须选择/移走旧报告，防止把旧成功结果当新结果。此检查验证窗口创建与原生渲染提交，不代表物理显示、IME 候选框或完整主应用端到端验收。

macOS 的真实后端契约可直接运行：

```sh
export AGENTKIT_BACKEND_DIR="$PWD/target/backend/darwin-arm64"
cargo test -p agentkit-sidecar --test agent_turn --locked -- --ignored
```

Windows PowerShell：

```powershell
$env:AGENTKIT_BACKEND_DIR = (Resolve-Path target/backend/win32-x64).Path
cargo test -p agentkit-sidecar --test agent_turn --locked -- --ignored
cargo build -p agentkit-ui --features samples --example components --locked
node tools/native-smoke/run.mjs
```

CI 中 macos-14 指 Apple Silicon arm64，windows-2022 指 x64；macOS Intel、Windows arm64 暂无本轮验证证据。Linux 本轮图形证据来自 X11/Xvfb，Wayland 实机仍待验证。

## macOS 应用包与 DMG 试跑

```sh
cargo xtask package --target macos-arm64 --sign ad-hoc
```

需原生 macOS 与 Xcode 命令行工具；版本源为根 Cargo.toml，品牌源为 apps/workbench/brand.toml。输出 target/packages/macos-arm64 中的 .app、DMG、签名与体积报告、SHA256SUMS；已存在输出不会被覆盖。应用从自身 Contents/Resources/backend 读取后端，缺失时不退回构建机源码路径。

PR 的 macOS package trial 工作流显式增加 --probe，以工程包运行隔离的原生工具回合；默认打包不启用探针。工作流挂载DMG并移走源码后端，检查签名/哈希、真实Read和最终帧，以及挂载后端的持久化/取消契约。自动挂载探针只在Actions运行，不作为普通本地启动器。

Developer ID与公证命令、凭据前提及完整限制见 [打包工具](../tools/package/README.md)；结果见 [T-0-09验证](verification-macos-package-2026-10-31.md)。当前ad-hoc工程包不等同于签名公证发行包，阶段0门槛仍未通过。
