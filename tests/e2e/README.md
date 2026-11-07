# 原生端到端与 P0 验收

```sh
cargo xtask backend build
cargo build -p workbench --features e2e --locked
node tests/e2e/guard.mjs target/debug/workbench
node tests/e2e/run.mjs target/debug/workbench target/backend/linux-x64 target/e2e/report.json
```

需要真实图形会话。Linux 可用 Xvfb/lavapipe，需 Python3 和 xdotool；macOS 使用 darwin-arm64，Windows 使用 win32-x64 与 workbench.exe。macOS 异常退出的独立进程组由每作业销毁的虚拟机兜底，Windows由owned Job管理，禁止在持久个人macOS桌面执行故障测试。CI的托管runner均在作业结束/取消后销毁。

测试启动完整原生应用，驱动先等待真实 Composer 的 model/session/操作状态就绪，再通过可选 e2e 构建操作正常控件的语义入口，包括 InputState 变更、Composer submit、连接表单 Test/Save、审批事件与正常取消。全部业务经正式 controller/API、认证 WebSocket、固定后端与Pi执行；不预建认证连接、不使用perf引导绕过、不注入会话事件或让模型代为执行文件操作。实际原生 Present 是必要证据；语义操作不代替物理鼠标键盘、输入法或无障碍验收。

本地模型提供可发现模型、拒绝测试端点、Read/Write工具请求、慢流与继续响应。真正的 Read/Write 由后端执行；模型验证Read标记，runner验证允许/拒绝/始终允许的文件副作用。后端 v0.2.6 的 compact transcript 不持久化临时工具消息；快照比较限定用户/助手正文，使用真实 user_message 的乐观ID→后端ID映射。身份、角色、正文仅在内存比较；另以应用进程重启后的7条用户消息各一次及5条确定性回复验证磁盘恢复，不写入测试报告。

崩溃注入通过监督器持有的子进程句柄杀死后端leader，正常actor负责回收其进程树、重启和新token。测试不寻找裸PID，不直接调用重启方法冒充崩溃。正常构建不包含该故障接口与UI驱动。

四次启动覆盖全流程、认证profile恢复、首次引导延期与延期恢复。首次profile必须为空且有专用标记，禁止远程附着或同时启用perf，所有输出独占创建。报告只含检查、计数、阶段与耗时；原始会话、模型地址、凭据、token、临时路径均不上传。

[p0.json](p0.json)逐项列出规格中的20项P0、已有自动化范围与剩余验收条件。`passed`仅表示本脚本覆盖的自动化流程成功；P0整体、实体桌面、参考机性能、正式发布始终单独记录。F-39在阶段3尚未实现，不能将阶段1测试成功签署为全部P0验收通过。

P-04 将连接探测限制在专用临时目录；P-05 以私有活动回合标记恢复崩溃中断提示，按会话串行化显式持久化边界，保存失败保留恢复证据并回滚发送 reservation。监督器将已退出的孤儿回收与强制终止活进程区分，活进程残留仍判失败。
