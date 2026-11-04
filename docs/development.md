# 当前开发与验证

## 工具链

Rust 1.99.1（rust-toolchain.toml）、Bun >=1.3.14、Node >=18、uv。GPUI Kit v0.8.1 上游 CI 使用 stable；本次安装当日 stable 为 1.99.1，已精确锁定。所有 Rust 依赖由 Cargo.lock 固定。

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

T-1-04 真实客户端契约：先运行 `cargo xtask backend build --contract`，设置 `AGENTKIT_BACKEND_DIR` 及 `AGENTKIT_CONTRACT_SERVER` 后运行 `cargo test -p agentkit-client --test contract --locked -- --ignored`；完整命令和平台目录见 [契约说明](../tools/contract/README.md)。普通 cargo test 显式跳过这些进程测试；CI contract 三平台运行。

T-1-05 进程监督测试使用同一后端工程包运行 `cargo test -p agentkit-sidecar --test manager --locked -- --ignored --skip owned_supervisor_fixture --skip shell_probe_fixture`，设置 `AGENTKIT_BACKEND_DIR`。见 [sidecar说明](../crates/agentkit-sidecar/README.md)。

T-1-09 真实事件录制与回放命令、14类场景及比较规则见 [录制说明](../tools/fixtures/README.md)。`AGENTKIT_RECORD_EVENTS` 为显式开发录制开关，目标必须是新文件，保留消息/工具内容并脱敏连接 token。

T-1-10 本地偏好与实例隔离见 [验证报告](verification-shell-2026-11-03.md)。`AGENTKIT_APP_DATA_DIR` 指定客户端prefs/socket目录；单独设置`AGENTKIT_CONFIG_DIR`时客户端默认使用其`client`子目录。真实亮暗窗口与第二次启动聚焦冒烟见 [shell-smoke](../tools/shell-smoke/README.md)。

T-1-11 文案修改后运行 `cargo xtask i18n` 并提交生成资源；`cargo xtask i18n --check` 只读验证。资源位置、命名插值及AST策略见 [i18n管线](../tools/i18n/README.md)。

T-1-12 对话组件、缓存/阶段与错误操作见 [验证报告](verification-chat-2026-11-03.md)。原生组件测试：`cargo test -p agentkit-ui --features samples --test transcript`。

T-1-13 工具卡、Diff与完整只读内容见 [验证报告](verification-tools-2026-11-03.md)。原生组件测试：`cargo test -p agentkit-ui --features samples --test tools --locked`；长输出预览限制不改变会话中的原文。

T-1-14 输入与会话选项见 [验证报告](verification-composer-2026-11-04.md)。原生输入协议与菜单测试：`cargo test -p agentkit-ui --features samples --test composer --locked`。发送键保存在客户端prefs的`send_key`，可在输入工具栏切换；完整后端草稿持久化按T-2-08推进。

T-1-15 权限审批见 [验证报告](verification-permissions-2026-11-04.md)。原生专项：`cargo test -p agentkit-ui --features samples --test permissions --locked`；真实后端及原生窗口命令见 [审批验证工具](../tools/permissions/README.md)。`approval_notifications` 控制本地后台审批提醒；完整通知设置按 T-2-15 推进。

T-1-16 原生侧栏与工作区切换见 [验证报告](verification-sidebar-2026-11-04.md)。专项命令与千条真实会话窗口工具见 [导航验证](../tools/sidebar/README.md)；默认不包含正文搜索、视图筛选或多窗口操作。

## 原生设置（T-1-17）

Ctrl/Cmd+, 或工作台设置按钮打开通用/外观/模型页；本地偏好立即保存，连接表单显式保存。现有密钥只提供替换入口，不读取明文。OAuth 连接的检查只验证已保存认证，登录归后续任务。

```sh
cargo test -p agentkit-ui --features samples --test settings --locked
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" cargo test -p agentkit-sidecar --test settings --locked -- --ignored
cargo build -p agentkit-sidecar --example settings-fixture --locked
node tools/settings/smoke.mjs target/debug/workbench target/backend/linux-x64 target/debug/examples/settings-fixture target/settings-light.json light
```

最后一项需 Linux X11、xdotool、Python Pillow 和 1440×900 测试桌面，报告必须不存在。它使用全新临时配置和本地模型、清理所属进程，不访问用户凭据。真实模型测试需要 SDK 可写用户会话目录，受限云环境的只读 homedir 不能作为探测成功证据。验证边界见 [报告](verification-settings-2026-11-04.md)。

## 首次启动引导（T-1-18）

无认证连接时自动进入四步引导，测试成功后才可保存、设默认并开始会话。第 2 步可稍后设置，延期经后端回执后持久化；无模型时显示添加连接空态。订阅登录仍归后续任务。

```sh
cargo test -p agentkit-ui --features samples --test onboarding --locked
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" cargo test -p agentkit-sidecar --test onboarding --locked -- --ignored
node tools/onboarding/smoke.mjs target/debug/workbench target/backend/linux-x64 target/onboarding-light.json light
```

最后一项需要 Linux 1440×900 X11/Vulkan、xdotool、Python Pillow。报告必须不存在；使用全新隔离配置并监督清理进程，检查延期和重启，不访问用户凭据。真实模型探测需要 SDK 可写用户目录，本地受限环境的只读目录不作为模型可用证据。详见 [验证报告](verification-onboarding-2026-11-04.md)。

## 原生客户端能力（T-1-19）

```sh
cargo test -p agentkit-capabilities --locked
cargo xtask backend build --contract
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" AGENTKIT_CONTRACT_SERVER="$PWD/target/contract/linux-x64/transport.js" cargo test -p agentkit-capabilities --test rpc_contract --locked -- --ignored
cargo build -p agentkit-capabilities --example client_caps --locked
node tools/client-caps/smoke.mjs target/debug/examples/client_caps target/backend/linux-x64 target/contract/linux-x64/transport.js target/client-caps.json
```

最后一项是 Linux 真实确认窗口检查，需要显示环境、Vulkan、xdotool 和 Python Pillow；输出路径必须尚不存在。macOS/Windows 路径替换及三平台契约说明见 [能力验证](verification-client-caps-2026-11-04.md)。

## 原生菜单与 keymap（T-1-20）

客户端数据目录中的 keymap.json 保存用户覆盖，设置的“键盘快捷键”页可打开并重新加载。默认键、固定作用域及 JSON 格式见 [menus 工具说明](../tools/menus/README.md)。macOS 使用系统菜单，Windows/Linux 使用同一菜单树的 Kit AppMenuBar。阶段2/3命令明确禁用。

```sh
cargo test -p agentkit-app --lib --features perf --locked
cargo test -p agentkit-ui --features samples --test shell --test settings --test composer --locked
node tools/menus/smoke.mjs target/debug/workbench target/backend/linux-x64 target/menus.json
```

最后一项需要 Linux 桌面/Vulkan/xdotool/Pillow，输出必须尚不存在。验证范围见 [菜单与快捷键验证](verification-menus-2026-11-04.md)。
