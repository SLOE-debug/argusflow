import type { WorkflowFile, JsonValue } from "../workflow";
/** 与后端 SQLite 配置契约一致；密钥不属于可读配置。 */
export interface AiConfig {
  readonly endpoint: string;
  readonly model: string;
  readonly vision: boolean;
  readonly bailian: boolean;
  readonly max_rounds: number;
  readonly max_tools: number;
  readonly max_tokens: number;
  readonly timeout_seconds: number;
}
export interface ConfigView {
  readonly config: AiConfig;
  readonly has_key: boolean;
}
export interface AiProgress {
  readonly round: number;
  readonly stage: "analyzing" | "inspecting" | "validating";
}
export interface AiResult {
  readonly file: WorkflowFile | null;
  readonly analysis: {
    readonly summary: string;
    readonly replay_ready: false;
    readonly node_evidence: readonly {
      readonly node_id: string;
      readonly evidence_ids: readonly string[];
      readonly outcome: "confirmed" | "request_only" | "uncertain";
      readonly uncertainty: string | null;
    }[];
    readonly unresolved: readonly {
      readonly evidence_ids: readonly string[];
      readonly reason: string;
    }[];
    readonly required_bindings: readonly {
      readonly name: string;
      readonly kind: "input" | "resource";
      readonly reason: string;
    }[];
  };
  readonly metrics: {
    readonly rounds: number;
    readonly tool_calls: number;
    readonly image_pixels: number;
    readonly usage: readonly JsonValue[];
  };
}
