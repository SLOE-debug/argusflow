import type { JsonValue, ValueType } from "../model/contracts";
import { BOOL, TEXT } from "../model/expressions";

export type NodeCategory =
  "逻辑控制" | "数据" | "浏览器" | "桌面自动化" | "查询与操作";
export interface ConfigField {
  readonly key: string;
  readonly label: string;
  readonly type: "text" | "boolean" | "number" | "u64" | "aql";
  readonly initial: JsonValue;
  readonly optional?: boolean;
}
export interface TaskSpec {
  readonly id: string;
  readonly title: string;
  readonly category: NodeCategory;
  readonly description: string;
  readonly inputs: Readonly<Record<string, ValueType>>;
  readonly outputs: Readonly<Record<string, ValueType>>;
  readonly resources: Readonly<Record<string, string>>;
  readonly creates: Readonly<Record<string, string>>;
  readonly config: readonly ConfigField[];
}
const BROWSER = "automation.browser",
  PAGE = "automation.page",
  SOURCE = "automation.query_source";
const APPLICATION = "automation.application",
  WINDOW = "automation.window";
const query: ConfigField = {
  key: "query",
  label: "目标查询",
  type: "aql",
  initial: "",
};
const titleFields: readonly ConfigField[] = [
  {
    key: "title",
    label: "窗口标题",
    type: "text",
    initial: null,
    optional: true,
  },
  {
    key: "class_name",
    label: "窗口类名",
    type: "text",
    initial: null,
    optional: true,
  },
];
const match: ValueType = {
  type: "record",
  of: {
    source: TEXT,
    space: TEXT,
    frame: { type: "optional", of: TEXT },
    name: { type: "optional", of: TEXT },
    text: { type: "optional", of: TEXT },
    confidence: { type: "optional", of: { type: "float" } },
    bounds: { type: "list", of: { type: "float" } },
  },
};
/** 对齐当前 Rust 自动化端口；配置差异只存在于这一份目录。 */
export const TASKS: readonly TaskSpec[] = [
  {
    id: "browser.launch",
    title: "启动浏览器",
    category: "浏览器",
    description: "创建独立浏览器会话",
    inputs: { executable: TEXT },
    outputs: {},
    resources: {},
    creates: { browser: BROWSER },
    config: [
      { key: "headless", label: "无头模式", type: "boolean", initial: false },
    ],
  },
  {
    id: "browser.connect",
    title: "连接浏览器",
    category: "浏览器",
    description: "连接调试端点",
    inputs: { endpoint: TEXT },
    outputs: {},
    resources: {},
    creates: { browser: BROWSER },
    config: [],
  },
  {
    id: "browser.pages",
    title: "列出页面",
    category: "浏览器",
    description: "读取浏览器页面清单",
    inputs: {},
    outputs: {
      pages: {
        type: "list",
        of: { type: "record", of: { id: TEXT, title: TEXT, url: TEXT } },
      },
    },
    resources: { browser: BROWSER },
    creates: {},
    config: [],
  },
  {
    id: "browser.attach",
    title: "附加页面",
    category: "浏览器",
    description: "选择已有页面",
    inputs: { target_id: TEXT },
    outputs: {},
    resources: { browser: BROWSER },
    creates: { page: PAGE },
    config: [],
  },
  {
    id: "browser.new_page",
    title: "新建页面",
    category: "浏览器",
    description: "打开网址",
    inputs: { url: TEXT },
    outputs: {},
    resources: { browser: BROWSER },
    creates: { page: PAGE },
    config: [],
  },
  {
    id: "browser.navigate",
    title: "页面导航",
    category: "浏览器",
    description: "在已有页面访问网址",
    inputs: { url: TEXT },
    outputs: {},
    resources: { page: PAGE },
    creates: {},
    config: [],
  },
  {
    id: "source.dom",
    title: "页面查询来源",
    category: "浏览器",
    description: "使用页面元素定位",
    inputs: {},
    outputs: {},
    resources: { page: PAGE },
    creates: { source: SOURCE },
    config: [],
  },
  {
    id: "application.launch",
    title: "启动应用",
    category: "桌面自动化",
    description: "启动并管理应用进程",
    inputs: { executable: TEXT, arguments: { type: "list", of: TEXT } },
    outputs: {},
    resources: {},
    creates: { application: APPLICATION },
    config: [
      { key: "visible", label: "显示窗口", type: "boolean", initial: true },
    ],
  },
  {
    id: "application.wait_window",
    title: "等待应用窗口",
    category: "桌面自动化",
    description: "定位已启动应用的窗口",
    inputs: {},
    outputs: {},
    resources: { application: APPLICATION },
    creates: { window: WINDOW },
    config: titleFields,
  },
  {
    id: "window.attach",
    title: "附加窗口",
    category: "桌面自动化",
    description: "按进程定位窗口",
    inputs: { process_id: { type: "int" } },
    outputs: {},
    resources: {},
    creates: { window: WINDOW },
    config: titleFields,
  },
  {
    id: "window.activate",
    title: "激活窗口",
    category: "桌面自动化",
    description: "将窗口置于前台",
    inputs: {},
    outputs: {},
    resources: { window: WINDOW },
    creates: {},
    config: [],
  },
  {
    id: "source.uia",
    title: "窗口查询来源",
    category: "桌面自动化",
    description: "使用桌面元素定位",
    inputs: {},
    outputs: {},
    resources: { window: WINDOW },
    creates: { source: SOURCE },
    config: [],
  },
  {
    id: "source.host",
    title: "宿主查询来源",
    category: "查询与操作",
    description: "使用已配置的共享来源",
    inputs: {},
    outputs: {},
    resources: {},
    creates: { source: SOURCE },
    config: [{ key: "name", label: "来源名称", type: "text", initial: "" }],
  },
  {
    id: "aql.query",
    title: "查询元素",
    category: "查询与操作",
    description: "读取匹配元素",
    inputs: {},
    outputs: { matches: { type: "list", of: match } },
    resources: { source: SOURCE },
    creates: {},
    config: [query],
  },
  {
    id: "aql.exists",
    title: "检查元素存在",
    category: "查询与操作",
    description: "返回存在状态",
    inputs: {},
    outputs: { exists: BOOL },
    resources: { source: SOURCE },
    creates: {},
    config: [query],
  },
  {
    id: "aql.wait",
    title: "等待元素",
    category: "查询与操作",
    description: "等待出现或消失",
    inputs: {},
    outputs: { exists: BOOL },
    resources: { source: SOURCE },
    creates: {},
    config: [
      query,
      {
        key: "interval_ms",
        label: "检查间隔（毫秒）",
        type: "u64",
        initial: "200",
      },
      { key: "present", label: "等待出现", type: "boolean", initial: true },
    ],
  },
  {
    id: "aql.click",
    title: "点击元素",
    category: "查询与操作",
    description: "重新定位并点击唯一目标",
    inputs: {},
    outputs: {},
    resources: { source: SOURCE },
    creates: {},
    config: [query],
  },
  {
    id: "aql.type_text",
    title: "输入文字",
    category: "查询与操作",
    description: "向定位目标输入",
    inputs: { text: TEXT },
    outputs: {},
    resources: { source: SOURCE },
    creates: {},
    config: [query],
  },
];
export const CONTROL_NODES = [
  {
    id: "let",
    title: "声明变量",
    category: "数据",
    description: "定义名称、类型和初始值",
  },
  {
    id: "assign",
    title: "赋值",
    category: "数据",
    description: "更新已有变量",
  },
  {
    id: "if",
    title: "条件分支",
    category: "逻辑控制",
    description: "根据条件选择执行路径",
  },
  {
    id: "switch",
    title: "多路分支",
    category: "逻辑控制",
    description: "根据值选择分支",
  },
  {
    id: "for_each",
    title: "遍历列表",
    category: "逻辑控制",
    description: "依次处理每个元素",
  },
  {
    id: "while",
    title: "条件循环",
    category: "逻辑控制",
    description: "条件满足时重复执行",
  },
  {
    id: "block",
    title: "步骤分组",
    category: "逻辑控制",
    description: "在独立作用域组织步骤",
  },
  {
    id: "wait",
    title: "等待",
    category: "逻辑控制",
    description: "等待指定毫秒数",
  },
  {
    id: "call_workflow",
    title: "调用工作流",
    category: "逻辑控制",
    description: "引用另一份独立工作流",
  },
  {
    id: "break",
    title: "退出循环",
    category: "逻辑控制",
    description: "结束当前循环",
  },
  {
    id: "continue",
    title: "下一轮",
    category: "逻辑控制",
    description: "继续当前循环",
  },
  {
    id: "return",
    title: "返回结果",
    category: "数据",
    description: "结束流程并返回数据",
  },
  {
    id: "fail",
    title: "终止并报错",
    category: "逻辑控制",
    description: "明确停止当前流程",
  },
  {
    id: "release",
    title: "释放资源",
    category: "数据",
    description: "提前关闭当前作用域资源",
  },
] as const;
export const ENDPOINT_NODES = [
  {
    id: "start",
    title: "开始",
    category: "逻辑控制",
    description: "流程入口，每个流程只能添加一个",
  },
  {
    id: "end",
    title: "结束",
    category: "逻辑控制",
    description: "流程出口，每个流程只能添加一个",
  },
] as const;
export const NODE_CATALOG = [...ENDPOINT_NODES, ...CONTROL_NODES, ...TASKS];
export function taskSpec(id: string): TaskSpec | undefined {
  return TASKS.find((item) => item.id === id);
}
export const PORT_LABELS: Readonly<Record<string, string>> = {
  executable: "程序路径",
  endpoint: "调试端点",
  target_id: "页面 ID",
  url: "网址",
  arguments: "启动参数",
  process_id: "进程 ID",
  text: "文字",
  browser: "浏览器",
  page: "页面",
  source: "查询来源",
  application: "应用",
  window: "窗口",
};
