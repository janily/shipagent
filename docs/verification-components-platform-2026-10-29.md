# T-0-10 组件样板与平台验证

本轮实现六类独立 GPUI Kit 原生组件样板，并根据最新用户要求恢复 macOS、Windows 验证。阶段 0 尚未通过。

## 组件与 Linux 结果

`cargo run -p agentkit-ui --features samples --example components --locked`，无需启动后端。samples feature 默认关闭，样板不访问真实会话、配置或密钥。

| 组件 | 行为与证据 |
| --- | --- |
| Sidebar | 工作区、会话入口、设置入口；返回后保留草稿 |
| Textarea | 中文输入/发送、Enter、Shift+Enter、空白提交不清草稿 |
| Collapsible | 工具摘要、展开和折叠结果 |
| Settings | 设置替换主区、原生主题开关即时生效，无保存按钮 |
| Dialog | 清空确认；取消保留消息/草稿，确认清空 |
| Resizable | 主区最小 400px，右栏 200–360px；原生拖动测试 |

Linux Xvfb + lavapipe 实窗启动、1280×820 和 960×600 走查通过；暗色设置开关实际点击通过。修正走查发现的主列未占满高度问题，增加最小尺寸与真实拖动回归。39 核心测试、10 原生测试（既有 7 + 样板 3）、全 workspace/all-targets/all-features clippy -D warnings 通过。Linux 实窗冒烟报告见下列 JSON。

- [浅色工作台](images/components-linux-light.png)
- [最小尺寸](images/components-linux-minimum.png)
- [暗色设置](images/components-linux-settings-dark.png)
- [Linux 原生提交报告](platform/linux-native-frame.json)
- [Windows 原生提交报告](platform/windows-native-frame.json)

## 三平台验证证据

| 主机 | 核心 / 原生控件测试 | 固定后端 / 真实 Agent 契约 | 实际窗口提交 |
| --- | --- | --- | --- |
| Linux x64 | 39 / 10 通过 | 通过 | X11/Xvfb + lavapipe；1 次 Present，1280×820 |
| macOS 14 arm64 | 39 / 10 通过 | 通过 | 1 次 Present，3 个回调后可见，1280×820 |
| Windows Server 2022 x64 | 39 / 10 通过 | 通过 | 1 次 Present，2 个回调后可见，1024×728 |

核心与三平台后端契约证据见 [CI #22](https://github.com/janily/shipagent/actions/runs/40965601470)，加强后的 macOS/Windows 原生窗口记录来自 [CI #23](https://github.com/janily/shipagent/actions/runs/40966883167)，原始记录在 docs/platform，包含提交与运行编号；Linux 为本地同版本样板实测。各平台还实际链接了主程序和组件样板。窗口报告验证该窗口的 GPUI 平台渲染调用完成，不代表物理屏幕扫描或 GPU 完成时间。

- [macOS 原生提交报告](platform/macos-native-frame.json)
- [完整矩阵最新结果](https://github.com/janily/shipagent/pull/5/checks)

CI 仅在 PR 和 main push 运行，避免功能提交重复启动三平台矩阵。macOS/Windows 冒烟使用原生平台，而非 test-support 平台；监督器要求正常退出、主机/架构/尺寸匹配及至少一次该窗口的 Present 记录，30 秒超时失败。physicalDesktopAcceptance 保持 not_evaluated。macOS Intel、Windows arm64 与 Linux Wayland 不在本轮实测范围。

## 验证工具修正

- 原大请求测试把 8MiB / 1 秒超时等同于写阻塞，Windows 上不成立；限制真实 socket 两端缓冲仍无法形成稳定假设。最终分三层：固定 4KiB AsyncWrite 夹具使真实 WebSocket 帧部分写入后阻塞，验证生产 controlled(write, deadline) 超时；真实 TCP 用例允许写入及时完成或按时超时，并核对已发帧头；真实 RPC 大请求用例要求按时返回、显式关闭后断开。没有改动生产网络语义，也没有添加测试专用生产钩子。
- 独立审查发现首个 on_next_frame 先于平台提交；实际复现 submittedFrames=0。仅固定等待两个回调仍在 macOS 失败。现在等待真实 Present 记录，最多 10 秒；macOS 在第 3 个回调取得提交，Linux/Windows 在第 2 个回调取得提交。新增 4 项报告测试拒绝零提交、旧格式、异机架构、尺寸不足等，并要求错误保留具体字段与 observed 观测。samples feature 启用 profiler，普通交互不打开 trace，默认主应用不启用 samples。
- Windows Agent 契约本身通过，但 uv 收尾缓存扫描 Bun 缓存报 EPERM，已关闭不必要的 uv 缓存。
- #23 的 macOS 后端下载遇到 ripgrep GitHub Release API 403。下载器明确支持 GITHUB_TOKEN，因此仅构建步骤传入内置只读 github.token；不传给 Agent 运行步骤，不打印令牌，不改变版本或下载校验。最新完整矩阵验证此配置。

本地核心 39、原生 10、Node 15（模型、性能工具和原生帧报告）、Python 3 项通过；全功能 clippy -D warnings、fmt 和 diff 检查通过。协议生成漂移由独立 CI 作业验证。

审查遗留两项 minor：独立样板交互模式窗口创建失败时尚未返回非零状态（冒烟模式会失败）；样板中文文案尚未接入 i18n。两项仅限开发样板，后续完善，未扩展为产品正式界面。

## 仍未覆盖的验收

- macOS Developer ID 签名/公证、Bun entitlements、DMG 体积与干净机安装（T-0-09）。本会话没有配置这些签名凭据，不以 ad-hoc 签名冒充公证。
- macOS/Windows 物理桌面 IME、系统窗口行为、辅助功能和完整应用端到端交互；CI 窗口冒烟与单独后端契约不可拼成主程序完整 U2 证据。
- Windows Job Object 进程树托管仍未实现。正常契约退出不证明崩溃时 Pi/工具进程无残留（T-1-05/T-3-01）。
- 真实 GPU/参考硬件帧预算、110–126ms 软件呈现延迟问题仍未关闭，见前一份 Linux 性能报告。
- T-0-11 阶段门槛仍缺上述证据，阶段 1–4 功能不在本轮宣称完成。
