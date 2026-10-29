# 阶段 0 首批验证记录

本记录是部分任务证据，不是阶段 0 通过报告，也不是 v1.0 完成声明。

## 已运行

- Linux x64：Rust 1.99.1，GPUI Kit 0.8.1，gpui-pre 0.3.10，Bun 1.3.14。
- 核心测试 21 项通过；真实后端测试在默认 suite 中 1 项 ignored，另外显式运行通过。
- 真实后端：隔离源码树 + P-03 + frozen bun install；server.js、自带 Bun、Pi 子进程、资源、rg、xlsx 和 LICENSE/NOTICE 产物检查通过；无 WebUI。
- 真实 smoke：临时 CONFIG_DIR 启动打包后端 → 认证握手 → server:getHealth → workspaces:get → 关闭 RPC → 进程组正常退出，约 3 秒。
- 核心 fmt / clippy（-D warnings）通过。
- 全 workspace cargo check 在补齐本地 Linux 开发库后通过（当时 UI/app 为空骨架）。
- cargo deny check 通过。维护公告与未用许可证允许项仍有提示；不等同于无风险。
- backend/mkagent 工作树保持干净。

## 本环境处理

终端默认权限下无法连接网络代理；使用环境提供的 network 权限后可访问允许的公开来源。Rust/Bun 及临时缓存安装在 workspace，不改 HOME。系统包管理目录不可写，Linux 开发包从官方 Debian 源下载并提取到工作目录，以 PKG_CONFIG_PATH 指向本地 pkgconfig。

上游 bun.lock 内 npm 镜像被网络策略拒绝，构建改用官方 npm。ripgrep 的 GitHub API 下载也被拒绝，通过已授权 GitHub 插件核实 release 元数据，再从允许的官方 release URL 缓存同一个产物。

## 尚未验证与完成

原生窗口/流式 UI、真实模型回复和工具回合、fake-llm、代码生成、组件样板、性能指标、macOS 干净机和签名公证均未完成。当前不能越过第 06 页的阶段 0 门槛进入 MVP 验收。GitHub CI 与代码审阅结果以实际运行记录为准，不由本地测试推定。

## 代码审阅与修复

独立审阅检查了握手错误传播、socket 背压、握手期间请求期限、子进程所有权和构建重复性。三项重要问题各有失败回归测试，修复后传输测试 9/9、全核心 suite 21/21；真实后端 smoke 再次通过。

- 握手 AUTH_FAILED、协议错误和连接超时保留到调用方及 last_error。
- 调用方独立计时；已过期/被取消的排队写入不会在握手后执行。
- 写入/flush/ack/close 有期限与关闭信号；背压导致不确定写入时终止连接，不重发。
- 把维护任务前移到每轮循环开头，避免本地拒绝请求绕过清理。

剩余次要项：极端线程资源耗尽时 stdout/stderr 线程创建和 Drop 清理线程需要更强的进程所有权兜底；发布产物替换期间进程被强杀可能只留下 .previous，现工具拒绝覆盖该备份并要求检查恢复。纳入后续生命周期/构建加固，不据此宣称生产级退出保证。

实施判定：共享 Endpoint 放在 protocol，避免 sidecar 的生产依赖反向依赖 client；Bun 镜像替换仅作用临时树；间接依赖维护公告单独跟踪而非作为漏洞豁免。
