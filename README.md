# ShipAgent

基于 Rust + GPUI Kit 的原生 Agent 桌面工作台，复用 mkagent 后端。

**当前处于阶段 0：已有可运行原生流式对话和真实工具回合，尚未达到发布验收。**

- [规格索引](docs/superpowers/specs/2026-10-29-shipagent-design.md)
- [完整开发规格](docs/spec/)
- [任务进度](docs/tasks.md)
- [开发与验证](docs/development.md)
- [首次验证记录](docs/verification-2026-10-29.md)
- [原生对话验证](docs/verification-native-turn-2026-10-29.md)
- [协议生成验证](docs/verification-codegen-2026-10-29.md)
- [Linux 流式性能测量](docs/verification-linux-perf-2026-10-29.md)
- [组件样板与三平台验证](docs/verification-components-platform-2026-10-29.md)

采用 AgentKit workspace 与 Workbench 参考应用，品牌配置集中管理；已纳入 Linux、macOS 与 Windows 原生 CI，实机交互和发布验收单独记录。第一版包含聊天、工具、审批、计划、模型连接与引导，不包含交互式浏览器。构建后端后使用 `cargo run -p workbench --locked` 启动，详见开发文档。
