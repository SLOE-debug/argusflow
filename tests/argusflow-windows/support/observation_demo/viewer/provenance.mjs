/** 人工核对代码后的来源说明；解释展示与事实采集的边界。 */
export const sourcePaths = {
  actor: "tests/argusflow-windows/support/observation_demo/actor.rs",
  scenario: "tests/argusflow-windows/support/observation_demo/model-run.mjs",
  driver:
    "tests/argusflow-windows/support/patchright_demo/observation-driver.mjs",
  listener: "tests/argusflow-windows/support/observation_demo/observe.rs",
  sampler: "tests/argusflow-windows/support/observation_demo/samples.rs",
  cdp: "tests/argusflow-windows/support/patchright_demo/cdp-observer.mjs",
  observer: "crates/argusflow-browser/src/page/observation.js",
  structure: "src-tauri/src/recorder/evidence/structure.rs",
  clipboard: "src-tauri/src/recorder/evidence/clipboard.rs",
  services: "src-tauri/src/recorder/evidence/services.rs",
  compact:
    "tests/argusflow-windows/support/observation_demo/conversation/evidence.mjs",
  images:
    "tests/argusflow-windows/support/observation_demo/conversation/images.mjs",
};

export const capabilities = [
  {
    name: "键盘、鼠标与前台上下文",
    status: "正式录制器已接入代码",
    level: "core",
    fact: "events.jsonl 来自 InputListener；包含按下/释放、VK、坐标、QPC、窗口和输入来源。",
    boundary:
      "本次 Windows 注入输入标记为 ArgusFlow。浏览器的 CDP 注入不保证经过 Windows 低级钩子，不能据此声称录到了连续鼠标轨迹。",
    sources: ["listener"],
  },
  {
    name: "控件属性、文档与选区",
    status: "正式录制器已接入代码",
    level: "core",
    fact: "UiaRuntime.observe_target 返回控件、RuntimeId、TextPattern 文档与真实 UTF-16 选区。正式结构证据包含 text/text_change/runtime_id。",
    boundary:
      "正式链路是事件后采样；本 demo 则在采样任务后等待 600ms 再采样，不是完整选区事件订阅。空 AutomationId 不能当唯一定位。",
    sources: ["structure", "sampler"],
  },
  {
    name: "剪贴板文本与系统序号",
    status: "正式录制器已接入代码",
    level: "core",
    fact: "本次复用 ClipboardReader，只读系统剪贴板；五次文本变化来自真实快照。",
    boundary:
      "序号变化不单独证明某个按键导致复制；正式链路在非敏感、非过期的结构观察后读取。本 demo 的缓存与调度不同。",
    sources: ["clipboard", "services", "sampler"],
  },
  {
    name: "浏览器 DOM 事件与选区",
    status: "基础能力已有；本次由 demo 接线",
    level: "demo",
    fact: "复用 argusflow-browser 的 observation.js。Node 通过真实 CDP 注入监听器，每 250ms 取一次队列。",
    boundary:
      "正式 recorder 的 EvidenceServices 当前只装配 UIA/剪贴板。本次没有保存经唯一性验证的 CSS 选择器；DOM 节点号不是跨会话定位器。",
    sources: ["observer", "cdp", "services"],
  },
  {
    name: "窗口截图与 OCR",
    status: "基础能力已有；本次由 demo 接线",
    level: "demo",
    fact: "samples.jsonl 的 image 与 ocr 来自窗口采集和 OcrEngine。相同帧复用同一文件。正式结构链路的画面由独立视频链路负责。",
    boundary:
      "本次截图、UIA、OCR 在采样区间内依次读取，并非原子快照；OCR 结果可能存在噪声。本次 OCR JSON 不能等同于正式录制器已经持久化的 OCR 证据。",
    sources: ["sampler", "services"],
  },
  {
    name: "易读时间线、行号与变化图",
    status: "离线确定性整理",
    level: "derived",
    fact: "键码转键名、时钟换算、前后值比较、UTF-16 选区换算行号以及图像差分由代码整理。",
    boundary:
      "这些是展示计算，不是 OS 直接报告的“复制第二行”意图；查看器保留原始 JSON，未把 AI 推断改写成原始事件。",
    sources: ["compact", "images"],
  },
  {
    name: "业务动作与 workflow 草稿",
    status: "模型推断，尚未验证回放",
    level: "ai",
    fact: "Plus/Max 从时间线和按需查询推断动作。此页面展示已经保存的回答，不调用模型。",
    boundary:
      "“保存请求”不等于磁盘写入成功；本次 11 项核对只是核心复制/粘贴/保存请求，不表示每个辅助步骤和定位都正确。",
    sources: ["compact"],
  },
];

export const presets = [
  {
    title: "执行脚本预设",
    text: "测试页面上的三个中文条目、li span 选择器、浏览器条目索引 0/1/2，以及最后先复制第二行再复制第一行，全部在 model-run.mjs 中预设。它们是模拟操作者的计划，不能充当录制事实。",
    sources: ["scenario"],
  },
  {
    title: "执行方法预设",
    text: "浏览器按 Alt 拖选，鼠标移动分 30 步，再按 Ctrl+C。记事本采用 Ctrl+End、Ctrl+V、Enter、Ctrl+S；选行采用 Ctrl+Home、方向键、Home、Shift+End、Shift+Left。窗口标题、编辑控件查询和等待时长也是 demo 的执行约束。",
    sources: ["actor", "driver"],
  },
  {
    title: "采集范围预设",
    text: "本次通过 ARGUSFLOW_ISOLATED_RECORDING 限制为 ArgusFlow Evidence 浏览器窗口和标题含 Observed- 的记事本。采集器还包含 Notepad 文档查询、应用类型判断、500 帧/1GiB 上限与 OCR 采样分支。它不是完全无应用知识的通用采集器。",
    sources: ["sampler", "listener"],
  },
  {
    title: "已隔开的数据流",
    text: "观察进程入口不接收执行计划；它监听系统事件并独立采样。模型输入构造器读录制文件，不读取 actor-private.json。查看器为便于审计才单独加载计划，并将其放在独立页面。代码结构支持这一隔离，但本次模拟并不能单独证明任意人工操作都已覆盖。",
    sources: ["listener", "compact", "scenario"],
  },
];
