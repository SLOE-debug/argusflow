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

/** 批量读取百度热搜标题与链接的 UI 节点 ID。 */
const COLLECT_NEWS_NODE_ID = 'collect_baidu_news_1';

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
 * 打开隔离 Chrome、导航百度、批量获取热搜标题与绝对链接并写入桌面 TXT。
 *
 * CollectLinks 在页面内一次性完成 DOM 查询和投影，每条记录固定为
 * `标题<TAB>链接<CRLF>`；PowerShell 只负责解析当前用户桌面并按 UTF-8 原样落盘。
 */
export const DEFAULT_NODES = [
  {
    id: 'start_1',
    kind: 'start',
    position: { x: 28, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.start },
    data: { kind: 'start', label: '开始', runState: 'idle' },
  },
  {
    id: BAIDU_BROWSER_NODE_ID,
    kind: 'browser',
    position: { x: 188, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.browser },
    data: {
      kind: 'browser',
      label: '打开 Chrome 并访问百度',
      spec: {
        executable_path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
        initial_url: 'https://www.baidu.com/',
        launch_timeout_ms: 15_000,
      },
      runState: 'idle',
    },
  },
  {
    id: 'wait_baidu_ready_1',
    kind: 'delay',
    position: { x: 402, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.delay },
    data: {
      kind: 'delay',
      label: '等待百度热搜渲染',
      milliseconds: 1_500,
      runState: 'idle',
    },
  },
  {
    id: COLLECT_NEWS_NODE_ID,
    kind: 'ui',
    position: { x: 586, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '批量获取热搜标题和链接',
      operation: {
        type: 'collect_links',
        target: createBaiduCdpTarget(
          'css("#hotsearch-content-wrapper a.title-content .title-content-title")',
        ),
      },
      runState: 'idle',
    },
  },
  {
    id: WRITE_NEWS_NODE_ID,
    kind: 'command',
    position: { x: 794, y: 104 },
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
          ].join('; '),
        },
        working_directory: null,
        environment: [],
        stdin: {
          type: 'node_output',
          node_id: COLLECT_NEWS_NODE_ID,
          output: 'text',
        },
        timeout_ms: 30_000,
        accepted_exit_codes: [0],
        max_stdout_bytes: 4_096,
        max_stderr_bytes: 65_536,
      },
      runState: 'idle',
    },
  },
  {
    id: 'debug_output_path_1',
    kind: 'debug',
    position: { x: 586, y: 238 },
    size: { ...WORKFLOW_NODE_SIZES.debug },
    data: {
      kind: 'debug',
      label: '输出保存路径',
      value: {
        type: 'node_output',
        node_id: WRITE_NEWS_NODE_ID,
        output: 'stdout',
      },
      runState: 'idle',
    },
  },
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 794, y: 238 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板按两行蛇形布局串联完整数据路径。 */
export const DEFAULT_EDGES = [
  createDefaultEdge('edge_start_browser', 'start_1', BAIDU_BROWSER_NODE_ID),
  createDefaultEdge(
    'edge_browser_wait',
    BAIDU_BROWSER_NODE_ID,
    'wait_baidu_ready_1',
  ),
  createDefaultEdge(
    'edge_wait_collect',
    'wait_baidu_ready_1',
    COLLECT_NEWS_NODE_ID,
  ),
  createDefaultEdge(
    'edge_collect_write',
    COLLECT_NEWS_NODE_ID,
    WRITE_NEWS_NODE_ID,
  ),
  createDefaultEdge(
    'edge_write_debug',
    WRITE_NEWS_NODE_ID,
    'debug_output_path_1',
    'bottom',
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
