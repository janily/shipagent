# T-0-09 macOS 工程包试跑

app/DMG 工具、显式 ad-hoc 签名、Bun 权限探针和挂载后的原生 Agent 回合已通过。**T-0-09 仍缺实际 Developer ID、公证和干净机验收，阶段0不标通过。**

命令和凭据前提见 [打包工具](../tools/package/README.md)，执行计划见 [计划](superpowers/plans/2026-10-31-macos-package.md)。

## 已测量的工程包

下表与 [原始摘要](platform/macos-package-2026-10-31.json) 来自 [试跑 #3](https://github.com/janily/shipagent/actions/runs/41192716141)，用于保留一次实际测量。后续重建 DMG 的时间/文件系统元数据可能改变哈希，以下载包自身 SHA256SUMS 为准。最新完整回归见 [PR #6 检查](https://github.com/janily/shipagent/pull/6/checks)。

| 项目 | 结果 |
| --- | --- |
| 主机 / 目标 | macOS14 Apple Silicon / arm64 |
| 版本 / 包类型 | 0.1.0 / 明确标注 engineering，启用隔离 perf 探针 |
| DMG 文件 | Workbench-0.1.0-macos-arm64-engineering.dmg |
| DMG 实际字节 | 68,024,869（约68.02MB，64.87MiB） |
| 体积预算 | 220,000,000字节；通过 |
| app 逻辑文件大小 | 163,068,243字节 |
| DMG SHA256 | 4c4b41ada0e5facdcdf9840a994ee82511fb7df9d6a6821203bd258e9c91d781 |
| 签署的 Mach-O | 主程序、Bun、rg、uv，共4个；逐一验证与整个app验证通过 |
| Bun entitlements | allow-jit、allow-unsigned-executable-memory |
| 其他程序 entitlements | 空；已读取实际签名核对 |
| 原生回合 | release/opt3；真实Read、29个text_delta、complete、最终140字节、完成帧已呈现 |
| 源码依赖隔离 | 源码后端临时移走、清除BACKEND_DIR、配置/工作目录置于独立临时路径，仍通过 |
| 挂载后端契约 | 持久化与取消测试通过 |
| 清理 | 源码后端恢复、DMG正常卸载通过 |

工程包产物可从 [该次Artifact](https://github.com/janily/shipagent/actions/runs/41192716141/artifacts/13039297250) 下载，保留14天；包含DMG、校验和和JSON。直接安装/测试DMG中的app，避免原始app经Artifact ZIP丢失Unix权限。此次没有发布GitHub Release，也没有上传Apple公证。

默认 `cargo xtask package --target macos-arm64 --sign ad-hoc` 不启用探针。CI显式加 `--probe` 的工程包复用正常Controller/后端/归约器和原生窗口流程，不用静态消息冒充回合；这不能替代默认生产构建的完整物理交互验收，也不作为性能基准。应用从自身 Contents/Resources/backend 查找后端，即使资源缺失也不回退构建机源码目录。

## 权限试验的结论与限制

该CI主机上，none、allow-jit、allow-jit+allow-unsigned-executable-memory 三种ad-hoc签名策略均观察到DFG编译1次并完成Wasm运算。**这种结果没有给出正式发行权限的有效下界，不能推导Developer ID包无需JIT权限。** 工程包保留文档规定的两项基线权限，不自动按候选试验减权。新增审计还会从CodeDirectory核对runtime标志位；签名标志存在不等于证明此CI主机执行了完整发行版安全策略。

仍需实际Developer ID/下载隔离环境和完整文档工具、插件加载覆盖，才能确定正式权限集；单次Read不证明所有工具权限完备。

## 自动化与修正

- 本地及CI验证：42项核心Rust、10项既有原生控件/指标、23项Node（含8项打包/回合/超时/签名元数据用例）、3项Python进程监督检查通过；全功能clippy、fmt/diff通过。协议生成漂移由CI独立检查。
- 首次release编译3分17秒，随后lipo参数顺序错误被真实工具拒绝；已按其用法修正，没有发布半成品。
- Windows不表达Unix执行位，仅该断言限于POSIX；其他布局/路径/权限清单校验仍在三平台运行。
- 独立审查后复现忽略SIGTERM的子进程可越过execFile软超时并成功返回；改为受控探针根进程SIGKILL，300ms回归通过。失败后代进程清理仍交给Actions跟踪清理/runner销毁，报告不声称已验证一般桌面崩溃清理。
- 内部Node入口重复参数/非布尔字符串的严格度作为minor后续完善；公开Cargo入口已在构建前严格校验，不受此项影响。

## 本轮取舍与剩余门槛

- 用明确标注的工程探针包完成自动原生回合；默认生产包不含探针，二者验收分开。
- 自动挂载验证仅用于Actions；失败清理记录为未验证，不推广成通用本地运行器。
- Developer ID证书行为、公证Accepted/staple、Gatekeeper/隔离属性、干净机、Intel/macOS13、全部插件权限仍无本轮证据。可选公证代码已准备，但当前没有相应凭据，未执行。
- Windows进程树、物理IME/辅助功能、参考硬件性能等既有阶段0/后续门槛未关闭。
