# T-1-05 sidecar 生命周期验证（2026-11-02）

基线 main208884a，PR11 已合并，后端仍固定 v0.2.6。监督器不依赖 GPUI；后台线程串行管理就绪、监控、主动重启、退避和退出。工作台监听 Restarted 更新已有 RPC 端点；Fatal 停止 RPC 重试并显示类型化原因。新身份仍要求缓存重载，自动快照合并/恢复 UX 属 T1-08/21。

实现与命令见 [sidecar README](../crates/agentkit-sidecar/README.md)。

- 意外退出发 Exited；一秒后重新启动，60秒内第四次发 Fatal；主动 restart 不消耗预算。AlreadyRunning 解析可选 PID 且不重试，未知 PID 不伪造零。
- 每代32随机字节 token；禁止额外环境覆盖 profile/token/local host/port，禁用 WebUI/TLS/health覆盖，补充资源 PATH。macOS 登录 shell5秒超时、缓存且可取消，Linux/Windows继承环境。
- Unix setsid/进程组 TERM→grace→KILL；Windows suspended→Job Object attach→resume，kill-on-close覆盖应用突退出。平台 API由固定版本的安全依赖封装，不在应用 crate 添加 unsafe。
- 双管道持续读取，日志2MiB阈值轮换保留一个旧文件、stderr尾64KiB、超大行整行省略；过滤token公告、脱敏token和密钥类额外环境。截断边界泄露片段由失败回归复现后修复。Unix日志权限0600。诊断文件写失败不阻塞管道，输出警告。
- 取消 start future / Drop / shutdown 都请求清理；退避可被取消；macOS shell查询可由停止标记中断。进程清理不等待逃逸进程持有的管道无限关闭。

本地六个真实进程用例已通过（含Unix后代宽限期flush回归）：崩溃新token与预算、占用/超时/主动重启、环境保护/广播/后代清理/退避关闭、真实后端锁冲突与崩溃后的RPC新身份、取消启动。重连测试先收到新端点，再等实际新握手后读取，保持代际屏障，不重发旧操作。

纯单测覆盖滚动窗口、日志脱敏/上限、轮换权限及跨截断边界 token 片段。原生13项和全feature Clippy另检查；最终三平台结果以 PR当前提交CI为准。CI中 macOS新增真实zsh捕获与shell导入/缓存/超时探针，Windows新增强杀监督进程后的Job树清理探针。

保留边界：Unix子进程主动脱离所属进程组（即使仍在同一session）不属于组清理保证；Windows直接终止job而无Unix信号宽限；完整单实例聚焦F03、退出确认界面、自动恢复UX及参考机F01冷启动/五秒恢复预算待后续验收。阶段0仍未通过，真实签名/公证和物理桌面/性能验收未完成，PR6工程包保留。

两轴审查发现并修复：Unix不能只看主进程退出就强杀全组，现保留后代宽限并以延迟flush夹具RED→GREEN；新增sidecar错误统一映射中英文资源，中文回归RED→GREEN。正常Quit先静默取消再断RPC、等待5秒；原有OS on_app_quit受GPUI约200ms预算限制，仍保留60ms快速清理兜底，完整退出确认交互尚待T1-21。

本地核心79项、原生13项、客户端真实契约3项、真实Agent1项和Linux监督用例6项作为本轮验证基线；最终CI检查三平台当前提交。
