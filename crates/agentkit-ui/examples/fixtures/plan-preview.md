## 用 Job Object 托管 sidecar 进程树

目标：主程序退出或崩溃时，sidecar、Pi 子进程及其工具子进程全部随之结束。

1. 在 `agentkit-sidecar` 中新增 `platform/windows.rs`，启动前创建 Job Object 并设置 `KILL_ON_JOB_CLOSE`。
2. 挂起启动 bun，加入 Job 后再恢复，避免子进程在加入前逃逸。
3. Unix 保持现有 setsid 进程组逻辑，抽出 `ProcessTree` trait 统一两端。
4. 新增集成测试：强制结束主进程后 5 秒内不得残留 bun 进程。

涉及 3 个文件，预计新增约 180 行。