# 原生连接恢复验证

`smoke.mjs` 在 Linux X11/Vulkan 的普通 workbench 窗口中运行，使用全新临时配置目录并移除模型凭据、远程端点和性能环境变量。

```sh
node tools/recovery/smoke.mjs target/debug/workbench target/backend/linux-x64 /tmp/recovery-new.json
```

脚本只终止本次独立配置目录 `.server.lock` 中记录的后端 PID。检查监督器启动新的后端、当前会话不变，且恢复后快捷键能创建新会话；保存重启前、重启中、恢复后的截图。退出报告必须显示原生退出码 0、没有强制清理或所属子进程残留。输出文件须不存在，测试配置自动删除。截图用于人工核对，脚本不把“新后端已就绪”单独认作界面恢复成功。

三平台 CI 另运行原生控制器契约：

```sh
AGENTKIT_BACKEND_DIR=target/backend/linux-x64 cargo test -p agentkit-app --lib recovery_contract_parent -- --ignored
```

契约在独立子进程中隔离环境与配置，验证后台审批缓冲、中断刷新、快速重连覆盖刷新、同一代际保留审批、真实后端替换后自动刷新、草稿/会话保留及列表去重。Windows 使用对应平台后端目录与 PowerShell 环境变量语法。

诊断导出当前为白名单恢复元数据 JSON，不含会话正文、路径、原始错误、令牌、日志或 API Key。完整日志 ZIP 归 T-2-17；实体桌面、参考机性能和正式签名公证仍需单独验收。
