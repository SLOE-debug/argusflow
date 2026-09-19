/** 录制协议 v2；所有序号在会话200万条原始记录预算内。 */
export type Phase =
  "Recording" | "Paused" | "Stopping" | "Stopped" | "Faulted" | "Interrupted";
export type Outcome =
  | "Complete"
  | "NotConfigured"
  | "Unavailable"
  | "Unresolved"
  | "TimedOut"
  | "BudgetExceeded"
  | "Stale"
  | "Cancelled"
  | "Failed";
export type Stage =
  | "Clipboard"
  | "Input"
  | "Journal"
  | "Uia"
  | "Cdp"
  | "Anchor"
  | "Visual"
  | "Ocr"
  | "Derivation";
export type Relation =
  "Input" | "Response" | "Before" | "After" | "Intermediate";
export interface Session {
  readonly id: string;
  readonly format: number;
  readonly created_ms: number;
  readonly qpc_origin: number;
  readonly qpc_frequency: number;
  readonly capture_session: string | null;
  readonly policy: string;
}
export interface RecordingEntry {
  readonly session: Session;
  readonly directory: string;
}
export interface RecorderStatus {
  readonly session: string | null;
  readonly phase: Phase | null;
  readonly elapsed_ms: number;
  readonly operations: number;
  readonly raw_count: number;
  readonly pending: number;
  readonly written: number;
  readonly synced: number;
  readonly input_fault: string | null;
  readonly evidence_gaps: number;
}
export interface WindowContext {
  readonly handle: number;
  readonly pid: number;
  readonly epoch: number;
}
export interface RawInput {
  readonly sequence: number;
  readonly qpc: number;
  readonly point: { readonly x: number; readonly y: number };
  readonly window: WindowContext;
  readonly foreground_handle: number;
  readonly origin: "System" | "InjectedUnknown" | "ArgusFlow";
  readonly flags: number;
  readonly system_time: number;
  readonly kind:
    | "Move"
    | "Context"
    | {
        readonly Button: {
          readonly button:
            "Left" | "Right" | "Middle" | { readonly Extra: number };
          readonly down: boolean;
        };
      }
    | {
        readonly Key: {
          readonly vk: number;
          readonly scan: number;
          readonly down: boolean;
          readonly repeat: boolean;
          readonly extended: boolean;
        };
      }
    | {
        readonly Wheel: {
          readonly horizontal: boolean;
          readonly delta: number;
        };
      };
}
export type InteractionKind =
  | "Click"
  | "DoubleClick"
  | "Drag"
  | "Scroll"
  | "Chord"
  | "SystemKey"
  | "TextUnconfirmed"
  | "ImeUnconfirmed"
  | "PasteUnconfirmed"
  | "DeleteUnconfirmed"
  | "TextObserved"
  | "WindowSwitch"
  | "Unresolved";
export interface Interaction {
  readonly raw: readonly number[];
  readonly kind: InteractionKind;
  readonly from_qpc: number;
  readonly through_qpc: number;
  readonly window: WindowContext;
  readonly basis: string;
  readonly related: number | null;
}
export type Property =
  | string
  | number
  | boolean
  | null
  | readonly Property[]
  | { readonly [key: string]: Property };
export interface Structure {
  readonly raw: readonly number[];
  readonly source: "Uia" | "Cdp";
  readonly relation: Relation;
  readonly from_qpc: number;
  readonly through_qpc: number;
  readonly identity: Readonly<Record<string, string>>;
  readonly properties: Readonly<Record<string, Property>>;
  readonly ancestors: readonly Readonly<Record<string, Property>>[];
  readonly bounds: readonly number[] | null;
  readonly truncated: boolean;
  readonly stale: boolean;
  readonly sensitive: boolean;
  readonly target_confirmed: boolean;
  readonly scope: string;
}
export interface PixelVersion {
  readonly session: string;
  readonly source: number;
  readonly generation: number;
  readonly revision: number;
}
export interface Attachment {
  readonly hash: string;
  readonly path: string;
  readonly bytes: number;
  readonly size: readonly [number, number];
}
export interface Visual {
  readonly raw: readonly number[];
  readonly relation: Relation;
  readonly version: PixelVersion;
  readonly presented_ns: number | null;
  readonly acquired_ns: number;
  readonly frozen_ns: number;
  readonly from_ns: number;
  readonly through_ns: number;
  readonly region: readonly number[];
  readonly screen_origin: readonly number[];
  readonly dpi: readonly number[];
  readonly status: "Anchored" | "Observed";
  readonly decision:
    | "SensitiveOmitted"
    | { readonly StructuredSufficient: string }
    | { readonly VisualRequired: string };
  readonly image: Attachment | null;
}
export interface Ocr {
  readonly raw: readonly number[];
  readonly image_hash: string;
  readonly version: PixelVersion;
  readonly screen_origin: readonly number[];
  readonly image_size: readonly [number, number];
  readonly screen_size: readonly [number, number];
  readonly blocks: readonly {
    readonly text: string;
    readonly confidence: number;
    readonly polygon: readonly (readonly number[])[];
  }[];
}
/** 剪贴板序号变化与输入只有时间关联；空文本、非文本、未变化分别记录。 */
export interface ClipboardObservation {
  readonly raw: readonly number[];
  readonly from_qpc: number;
  readonly through_qpc: number;
  readonly observation: {
    readonly previous_sequence: number | null;
    readonly sequence: number;
    readonly content:
      | "Unchanged"
      | "NoText"
      | {
          readonly Text: { readonly text: string; readonly truncated: boolean };
        };
  };
}
export type RecordData =
  | { readonly Clipboard: ClipboardObservation }
  | { readonly Raw: RawInput }
  | { readonly Interaction: Interaction }
  | { readonly Structure: Structure }
  | { readonly Visual: Visual }
  | { readonly Ocr: Ocr }
  | {
      readonly Association: {
        readonly records: readonly number[];
        readonly relation: string;
      };
    }
  | {
      readonly Attempt: {
        readonly raw: readonly number[];
        readonly stage: Stage;
        readonly outcome: Outcome;
        readonly reason: string;
      };
    }
  | { readonly State: { readonly phase: Phase; readonly reason: string } }
  | { readonly Durability: { readonly through: number } };
export interface RecordingRecord {
  readonly id: number;
  readonly written_qpc: number;
  readonly data: RecordData;
}
export interface Cursor {
  readonly offset: number;
  readonly sequence: number;
}
export interface JournalPage {
  readonly records: readonly RecordingRecord[];
  readonly cursor: Cursor;
  readonly tail: string | null;
  readonly end: boolean;
}

/** 时间线单项操作及其关联证据、相邻操作边界。 */
export interface OperationContext {
  readonly action: RecordingRecord;
  readonly records: readonly RecordingRecord[];
}
