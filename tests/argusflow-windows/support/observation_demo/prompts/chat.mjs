/** demo 实际注册的组合节点，不属于默认应用内置节点目录。 */
export const chatPrompt = `本 demo 额外注册 demo.wechat_paste_send（version=1，config={}，resources={}，inputs={recipient:text,expected:text}，输出 receipt:text）。
它复用项目已有微信能力：核对剪贴板等于 expected；激活微信；确认接收人；若不在当前会话则按列表 Home、最多三次 PageDown、最后 Ctrl+F 的顺序定位；输入框无草稿才执行真实 Ctrl+V、核对草稿、一次 Enter。发送前建立气泡基线，发送后跟踪唯一新生气泡并验证正文和草稿清空；结果不确定停止、不重发。当前授权仅文件传输助手。
此组合节点可以覆盖同一轮微信输入框点击、Control+V 和 Enter，必须同时引用该轮两个真实按键事件，以及显示接收人/草稿/后续消息的采样。expected 从记事本复制后的剪贴板证据推断，不读取示范计划。三次发送不能合并丢弃；应在各自真实复制之后发生。任务结束后微信在前台，回记事本前要 window.activate。
不要用 aql.press_keys 向微信发送，当前录制的微信未提供输入框 UIA；不要用额外文字输入代替真实复制。此节点是已实现的 demo 能力，尚非桌面应用默认内置节点。`;
