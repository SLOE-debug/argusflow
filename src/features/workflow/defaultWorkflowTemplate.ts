import type {
  AutomationTarget,
  JsonObject,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';

/** 默认示例中生产隔离 CDP 页面会话的 Browser 节点 ID。 */
const BAIDU_BROWSER_NODE_ID = 'baidu_browser_1';

/** 在已获取浏览器会话中访问百度的 Navigate 节点 ID。 */
const NAVIGATE_BAIDU_NODE_ID = 'navigate_baidu_1';

/** 批量读取百度热搜标题与链接的 UI 节点 ID。 */
const COLLECT_NEWS_NODE_ID = 'collect_baidu_news_1';

/** 把结构化热搜对象数组格式化为制表文本的节点 ID。 */
const FORMAT_NEWS_NODE_ID = 'format_baidu_news_1';

/** 把 CRLF 文本写入当前用户桌面的命令节点 ID。 */
const WRITE_NEWS_NODE_ID = 'write_baidu_news_1';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '采集百度热搜并写入桌面文本';

/** 默认模板不依赖预置变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 默认模板运行时不要求用户补充输入。 */
export const DEFAULT_WORKFLOW_INPUTS = [] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 没有输入声明时运行值保持空对象。 */
export const DEFAULT_RUN_INPUT_VALUES = {} as const satisfies JsonObject;

/** 浏览器启动和 PowerShell 文件写入是本示例需要的最小系统能力。 */
export const DEFAULT_WORKFLOW_PERMISSIONS = {
  allow: [
    'process.application.launch',
    'process.command.powershell',
  ],
} as const satisfies WorkflowPermissions;

/** 默认选中 Browser 节点，优先展示隔离 CDP 会话配置。 */
export const DEFAULT_SELECTED_NODE_ID = BAIDU_BROWSER_NODE_ID;

/**
 * 获取隔离 Chrome、显式导航百度、结构化提取并格式化热搜链接后写入桌面 TXT。
 */
export const DEFAULT_NODES = [
  {
    id: 'start_1',
    kind: 'start',
    position: { x: 28, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.start },
    data: { kind: 'start', label: '开始', outputBindings: {}, runState: 'idle' },
  },
  {
    id: BAIDU_BROWSER_NODE_ID,
    kind: 'browser',
    position: { x: 188, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.browser },
    data: {
      kind: 'browser',
      label: '获取 Chrome 会话',
      outputBindings: {},
      spec: {
        executable_path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
        acquire_mode: 'launch_isolated_cdp',
        launch_timeout_ms: 15_000,
        cleanup_policy: 'close_on_workflow_end',
      },
      runState: 'idle',
    },
  },
  {
    id: NAVIGATE_BAIDU_NODE_ID,
    kind: 'navigate',
    position: { x: 402, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.navigate },
    data: {
      kind: 'navigate',
      label: '访问百度',
      outputBindings: {},
      operation: {
        type: 'navigate',
        browser: {
          producer_node_id: BAIDU_BROWSER_NODE_ID,
          output_name: 'session',
        },
        url: { type: 'literal', value: 'https://www.baidu.com/' },
      },
      runState: 'idle',
    },
  },
  {
    id: COLLECT_NEWS_NODE_ID,
    kind: 'ui',
    position: { x: 610, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '提取热搜标题和链接',
      outputBindings: {},
      operation: {
        type: 'extract',
        target: createBaiduCdpTarget(
          'css("#hotsearch-content-wrapper a.title-content")',
        ),
        cardinality: 'many',
        fields: [
          { name: 'title', source: { type: 'text' } },
          { name: 'url', source: { type: 'attribute', name: 'href' } },
        ],
      },
      execution: {
        target_wait: {
          mode: 'bounded',
          timeout_ms: 5_000,
          poll_interval_ms: 100,
        },
      },
      runState: 'idle',
    },
  },
  {
    id: FORMAT_NEWS_NODE_ID,
    kind: 'format',
    position: { x: 610, y: 220 },
    size: { ...WORKFLOW_NODE_SIZES.format },
    data: {
      kind: 'format',
      label: '格式化热搜文本',
      outputBindings: {},
      operation: {
        items: {
          type: 'ref',
          source: { type: 'node', node_id: COLLECT_NEWS_NODE_ID },
          pointer: '/items',
        },
        fields: ['title', 'url'],
        column_separator: '\t',
        row_separator: '\r\n',
        include_header: false,
      },
      runState: 'idle',
    },
  },
  {
    id: WRITE_NEWS_NODE_ID,
    kind: 'command',
    position: { x: 402, y: 220 },
    size: { ...WORKFLOW_NODE_SIZES.command },
    data: {
      kind: 'command',
      label: '写入桌面百度热搜.txt',
      operation: {
        runner: 'power_shell',
        program: null,
        arguments: [],
        script: {
          type: 'literal',
          value: [
            '[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)',
            '[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)',
            '$desktop = [Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)',
            '$path = Join-Path $desktop "百度热搜.txt"',
            '$content = [Console]::In.ReadToEnd()',
            '[IO.File]::WriteAllText($path, $content, [Text.UTF8Encoding]::new($true))',
            '[Console]::Out.Write($path)',
          ].join('\n'),
        },
        working_directory: null,
        environment: [],
        stdin: {
          type: 'ref',
          source: { type: 'node', node_id: FORMAT_NEWS_NODE_ID },
          pointer: '/text',
        },
        timeout_ms: 30_000,
        accepted_exit_codes: [0],
        max_stdout_bytes: 4_096,
        max_stderr_bytes: 65_536,
      },
      runState: 'idle',
      outputBindings: {
        output: { type: 'expression', source: 'result.stdout' },
      },
    },
  },
  {
    id: 'debug_output_path_1',
    kind: 'debug',
    position: { x: 188, y: 336 },
    size: { ...WORKFLOW_NODE_SIZES.debug },
    data: {
      kind: 'debug',
      label: '输出保存路径',
      outputBindings: {},
      value: {
        type: 'ref',
        source: { type: 'node', node_id: WRITE_NEWS_NODE_ID },
        pointer: '/output',
      },
      runState: 'idle',
    },
  },
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 402, y: 336 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', outputBindings: {}, runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板按两行蛇形布局串联完整数据路径。 */
export const DEFAULT_EDGES = [
  createDefaultEdge('edge_start_browser', 'start_1', BAIDU_BROWSER_NODE_ID),
  createDefaultEdge('edge_browser_navigate', BAIDU_BROWSER_NODE_ID, NAVIGATE_BAIDU_NODE_ID),
  createDefaultEdge('edge_navigate_collect', NAVIGATE_BAIDU_NODE_ID, COLLECT_NEWS_NODE_ID),
  createDefaultEdge(
    'edge_collect_format',
    COLLECT_NEWS_NODE_ID,
    FORMAT_NEWS_NODE_ID,
    'bottom',
    'right',
  ),
  createDefaultEdge(
    'edge_format_write',
    FORMAT_NEWS_NODE_ID,
    WRITE_NEWS_NODE_ID,
    'left',
    'right',
  ),
  createDefaultEdge(
    'edge_write_debug',
    WRITE_NEWS_NODE_ID,
    'debug_output_path_1',
    'left',
    'right',
  ),
  createDefaultEdge(
    'edge_debug_end',
    'debug_output_path_1',
    'end_1',
  ),
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;

/** 为默认采集节点绑定 Browser.session 并强制使用 CDP 原生 CSS fast path。 */
function createBaiduCdpTarget(source: string): AutomationTarget {
  return {
    scope: {
      type: 'browser',
      resource: {
        producer_node_id: BAIDU_BROWSER_NODE_ID,
        output_name: 'session',
      },
    },
    locator: {
      type: 'query',
      query: { language_version: 1, source },
    },
    backend_policy: {
      allow: ['browser_cdp'],
      deny: [],
      prefer: ['browser_cdp'],
    },
  };
}

/** 创建默认示例中的无分支连线，并允许蛇形拐点显式指定锚点。 */
function createDefaultEdge(
  id: string,
  sourceNodeId: string,
  targetNodeId: string,
  sourceSide: WorkflowCanvasEdge['source']['side'] = 'right',
  targetSide: WorkflowCanvasEdge['target']['side'] = 'left',
): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: sourceNodeId, side: sourceSide },
    target: { nodeId: targetNodeId, side: targetSide },
    data: { branch: null },
  };
}
