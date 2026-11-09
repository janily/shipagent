# T-2-03 ChatGPT Plus OAuth

基线 PR #33 合并提交 5cc452c。依据 F-05 与 03·oauth_chatgpt 实现设置和首次引导入口。

- 先绑定 127.0.0.1:1455，再调用 startOAuth；只打开已校验的 OpenAI HTTPS 授权地址。
- 使用项目已有 async-net/smol 实现限长、限时 HTTP 回调；校验路径、Host、唯一参数和真实回调 state。错误/伪造回调不得完成授权。
- 成功仅在后端 complete/setup/auth status/catalog 读回确认后显示；不读取 token。取消、拒绝、超时、断线和窗口退出清理监听及后端 flow。
- 新建唯一 ChatGPT 连接；取消发生在交换期间时，等待交换结束再清理该新连接凭据，不污染已有连接。
- 界面与本地 HTTP/真实后端契约、双语、独立审查、三平台 CI、commit/push/PR。真实订阅浏览器登录不使用用户凭据自动化，不冒充真人验收。

阶段0/P0、正式签名及物理桌面验收保持开放。下一项 T-2-04 Claude OAuth。
