/** 录制器专用解析后端，与执行路由标识分离。 */
export type EvidenceBackend = 'managed_cdp' | 'uia';
/** 虚拟桌面物理像素点，允许负坐标。 */
export type ScreenPoint = Readonly<{ x: number; y: number }>;
/** 屏幕物理矩形。 */
export type InspectionRect = Readonly<ScreenPoint & { width: number; height: number }>;
/** AQL v3 支持的语义角色；对应 Rust ElementRole serde 名称。 */
export type ElementRole = 'window' | 'dialog' | 'pane' | 'button' | 'text_box'
  | 'check_box' | 'radio' | 'combo_box' | 'list' | 'list_item' | 'tree' | 'tree_item'
  | 'tab' | 'tab_item' | 'menu' | 'menu_item' | 'link' | 'image' | 'table' | 'row'
  | 'cell' | 'document' | 'text';
/** 实际输入窗口上下文；HWND 仅用于展示身份。 */
export type InspectionContext = Readonly<{
  /** 不透明 HWND 和用于检测句柄复用的 PID。 */
  window: Readonly<{ handle: number; process_id: number }>;
  /** 权限允许时返回实际 EXE 路径。 */
  executable_path: string | null;
  /** 根窗口标题，不来自输入字段值。 */
  title: string;
  /** 根窗口 Win32 类名。 */
  class_name: string;
  /** 根窗口屏幕物理范围。 */
  bounds: InspectionRect;
  /** 可确认的原生浏览器内容范围，缺失时禁止猜工具栏高度。 */
  browser_viewport: InspectionRect | null;
  /** 窗口 DPI，仅用于长度换算。 */
  dpi: number;
  /** 校验浏览器活动文档，不能替代点击窗口定位。 */
  has_keyboard_focus: boolean;
}>;
/** 后端观察的语义属性，不提供控件 value。 */
export type ElementSemantics = Readonly<{
  /** 可由 AQL 表达的有限角色。 */
  role: ElementRole | null;
  /** 可访问名称；敏感目标清空。 */
  name: string | null;
  /** UIA provider 提供的控件标识。 */
  automation_id: string | null;
  /** DOM data-testid。 */
  test_id: string | null;
  /** 观察到的 DOM id，不推断稳定性。 */
  stable_id: string | null;
  /** UIA ClassName 或 DOM class。 */
  class_name: string | null;
  /** UIA FrameworkId。 */
  framework_id: string | null;
}>;
/** 与 Rust InspectedEntity 一致的只读事实。 */
export type InspectedEntity = Readonly<{
  /** 本次录制期间比较目标身份，不用作 selector。 */
  identity: string;
  /** 当前目标的观察属性。 */
  semantics: ElementSemantics;
  /** 由近到远的有限祖先语义事实。 */
  ancestors: readonly ElementSemantics[];
  /** 屏幕物理范围。 */
  bounds: InspectionRect;
  /** 可编辑事实，不代表支持完整 SetValue。 */
  editable: boolean;
  /** 开启敏感输入遮盖时，未知字段也执行遮盖。 */
  sensitivity: 'normal' | 'sensitive' | 'unknown';
  /** 当前已附加的资源 ID；其他后端为空。 */
  browser_session: string | null;
  /** 已去掉 query、hash 和 userinfo 的页面地址。 */
  page_url: string | null;
  /** 0–1 观察置信度，不是回放成功率。 */
  confidence: number;
}>;
/** 后端失败的封闭分类。 */
export type InspectionFailure = 'unmanaged_window' | 'context_changed' | 'invalid_geometry'
  | 'no_element' | 'unavailable' | 'timeout' | 'unsupported_scope';
/** Trace 的输入与定位诊断。 */
export type RecordingDiagnostic =
  | Readonly<{ type: 'inspection_failed'; backend: EvidenceBackend; reason: InspectionFailure }>
  | Readonly<{ type: 'context_unavailable' | 'screenshot_unavailable'; reason: InspectionFailure }>
  | Readonly<{ type: 'keyboard_decode'; reason: KeyboardDecodeFailure }>
  | Readonly<{ type: 'late_inspection' | 'input_gap' | 'redacted' }>;
/** 不含原文字的键盘解码失败原因，与 Rust 契约保持一致。 */
export type KeyboardDecodeFailure = 'invalid_key' | 'unsupported_chord' | 'missing_window'
  | 'missing_thread' | 'input_method_active' | 'dead_key' | 'no_character';
/** PNG 图像与点击 crop；路径相对于演示包根目录。 */
export type ScreenshotEvidence = Readonly<{
  /** 操作后的画面是否在采样预算内稳定。 */
  stabilized: boolean;
  /** 按点击附近实际像素选择的高对比 RGB。 */
  click_color: readonly [number, number, number] | null;
  /** 完整可见窗口区域 PNG。 */
  path: string;
  /** 图像采样开始的相对毫秒数。 */
  captured_at_ms: number;
  /** 图像采样耗时，毫秒。 */
  capture_duration_ms: number;
  /** 图像对应的虚拟桌面物理范围。 */
  screen_bounds: InspectionRect;
  /** PNG 物理像素宽度。 */
  width: number;
  /** PNG 物理像素高度。 */
  height: number;
  /** 鼠标屏幕坐标，不是生成的执行目标。 */
  pointer: ScreenPoint | null;
  /** 局部 PNG 与其在完整图像内的范围。 */
  crop: Readonly<{ path: string; bounds: InspectionRect }> | null;
  /** 局部 PNG 失败不撤销完整窗口证据。 */
  crop_failure: InspectionFailure | null;
}>;
/** 结构化观察的来源与采样时间不可分离。 */
export type UiSnapshot = Readonly<{
  /** 实际采样后端。 */
  backend: EvidenceBackend;
  /** UIA/CDP 观察事实。 */
  entity: InspectedEntity;
  /** 开始采样的相对毫秒数。 */
  observed_at_ms: number;
  /** 采样耗时，毫秒。 */
  observation_duration_ms: number;
}>;
/** 一个事件的证据，不要求图像有对应元素。 */
export type EventEvidence = Readonly<{
  /** 点击时的目标像素，与随后出现的结果窗口分开。 */
  click_target: ScreenshotEvidence | null;
  /** 输入发生时采集的实际窗口上下文。 */
  context: InspectionContext | null;
  /** 缺少可靠结构化信息时为空。 */
  ui_snapshot: UiSnapshot | null;
  /** 操作后的主要图像，采样时间独立于输入与结构化观察。 */
  screenshot: ScreenshotEvidence | null;
  /** 按后端尝试次序保存失败原因。 */
  diagnostics: readonly RecordingDiagnostic[];
}>;
