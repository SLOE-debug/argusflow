import type { AqlQuery } from '../workflow';

/** 录制器专用解析后端，与执行路由标识分离。 */
export type ResolutionBackend = 'managed_cdp' | 'uia' | 'vision' | 'coordinate';
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
  /** DOM id，实际稳定性由合成器评分。 */
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
  /** 未知字段也必须遮盖输入。 */
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
  | Readonly<{ type: 'fallback'; backend: ResolutionBackend; reason: InspectionFailure }>
  | Readonly<{ type: 'keyboard_decode'; reason: KeyboardDecodeFailure }>
  | Readonly<{ type: 'late_inspection' | 'input_gap' | 'unsupported_input' | 'unpaired_mouse'
    | 'redacted' | 'selector_uniqueness_unverified' }>;
/** 不含原文字的键盘解码失败原因，与 Rust 契约保持一致。 */
export type KeyboardDecodeFailure = 'invalid_key' | 'unsupported_chord' | 'missing_window'
  | 'missing_thread' | 'input_method_active' | 'dead_key' | 'no_character';
/** 每个分值的观察依据；坐标明确降权。 */
export type CandidateBasis = 'automation_id' | 'test_id' | 'stable_id' | 'role_name'
  | 'stable_ancestor' | 'class' | 'dynamic_attribute' | 'visual_text' | 'coordinate';
/** 可回放 AQL 与物理坐标显式分离。 */
export type RecordedSelector = Readonly<{ type: 'aql'; value: AqlQuery }>
  | Readonly<{ type: 'coordinate'; value: ScreenPoint }>;
/** 确定性候选，分值 0–100 不表示唯一性。 */
export type SelectorCandidate = Readonly<{
  /** 合成器已生成的 AQL 或显式坐标。 */
  selector: RecordedSelector;
  /** 0–100 启发式稳定性。 */
  stability_score: number;
  /** 确定性分值依据。 */
  basis: CandidateBasis;
  /** 候选需要的定位后端。 */
  backend: ResolutionBackend;
}>;
/** 一次解析的完整快照。 */
export type ResolvedTarget = Readonly<{
  /** 输入发生时采集的实际窗口上下文。 */
  context: InspectionContext | null;
  /** 最终成功的解析后端。 */
  backend: ResolutionBackend;
  /** 坐标降级时缺少语义实体。 */
  entity: InspectedEntity | null;
  /** 按稳定性从高到低排列。 */
  selector_candidates: readonly SelectorCandidate[];
  /** 首选项在 candidates 内的索引，不重复保存 selector。 */
  preferred_selector: number | null;
  /** 0–1 定位观察置信度。 */
  confidence: number;
  /** 按后端尝试次序保存失败原因。 */
  diagnostics: readonly RecordingDiagnostic[];
}>;
