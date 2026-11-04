# T-1-19 客户端能力五项

基线：合并 PR #25 的 dc75e90。按 02 client:* 与 03 client_caps 实现 openExternal/openPath/showItemInFolder/confirmDialog/openFileDialog，握手只声明已安装的五项，浏览器能力保持不可用。注册在建连前；现有传输分派负责超时、断线和反向错误帧。

capabilities 层持有类型化参数校验、可取消的有界前台队列及原生实现；GPUI 线程选择最近聚焦窗口并显示 Kit AlertDialog/原生文件选择，后台执行阻塞 OS 调用。确认支持原始按钮序号、默认/取消和关闭；文件选择支持文件/目录/多选与取消，有默认路径/过滤器时使用带父窗口的原生 rfd 选择器补足当前 GPUI PathPromptOptions 缺失项；macOS 生产路径统一使用父窗口 sheet。无窗口常规确认使用 rfd，复杂按钮集由请求独占的临时 Kit 窗口保持完整序号、默认与取消语义。URL仅 http/https/mailto；路径为本地绝对路径。

验证参数/生命周期、原生确认与文件选择、超时/旧代际不执行排队动作、真实固定上游 transport 反向往返与错误，以及完整三平台 CI。实际系统外部应用和无头桌面局限单独记录，不冒充物理桌面验收。下一项 T-1-20；阶段0与签名/公证门槛仍开放。
