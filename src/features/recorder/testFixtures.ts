import type { CompletedRecording, RecorderStatus, RecordingSummary } from './model';

/** 脱敏 Trace fixture 同时用于 IPC 编排和真实组件测试。 */
export const RECORDING_FIXTURE: CompletedRecording = {
  files: { raw: 'D:\\recordings\\raw.json', normalized: 'D:\\recordings\\semantic.json', manifest: 'D:\\recordings\\manifest.json' },
  trace: {
    schema_version: 1, recording_id: '11111111-1111-4111-8111-111111111111',
    started_at_unix_ms: 1788746000000, dropped_events: 0,
    raw: { events: [{ sequence: 1, timestamp_ms: 100, elapsed_ms: 10,
      input: { type: 'key', virtual_key: null, scan_code: null, flags: null, phase: 'down',
        text: { type: 'redacted' }, chord: null }, target: null, diagnostics: [{ type: 'redacted' }] }] },
    normalized: { diagnostics: [], records: [{
      sequence: 1, started_ms: 10, ended_ms: 100, raw_event_ids: [1], original_input: [],
      operation: { type: 'type_text', text: { type: 'redacted' } },
      target: { context: { window: { handle: 123, process_id: 42 }, executable_path: 'D:\\Example.exe',
        title: '测试窗口', class_name: 'FixtureWindow', bounds: { x: 0, y: 0, width: 800, height: 600 },
        browser_viewport: null, dpi: 96, has_keyboard_focus: true },
      backend: 'uia', confidence: 0.9, preferred_selector: 0,
      diagnostics: [{ type: 'redacted' }, { type: 'selector_uniqueness_unverified' }],
      entity: { identity: 'uia:42', editable: true, sensitivity: 'sensitive', ancestors: [],
        bounds: { x: 10, y: 10, width: 100, height: 20 }, browser_session: null, page_url: null,
        confidence: 0.9, semantics: { role: 'text_box', name: null, automation_id: 'password',
          test_id: null, stable_id: null, class_name: 'Edit', framework_id: 'Win32' } },
      selector_candidates: [{ backend: 'uia', basis: 'automation_id', stability_score: 98,
        selector: { type: 'aql', value: { language_version: 3, source: 'textbox[uia.automation_id="password"]', bindings: {} } } }],
      },
    }] },
  },
};

/** 正在监听的控制面 fixture。 */
export const RECORDING_STATUS: RecorderStatus = {
  phase: 'recording', recording_id: RECORDING_FIXTURE.trace.recording_id,
  started_at_unix_ms: RECORDING_FIXTURE.trace.started_at_unix_ms, processed_events: 2, dropped_events: 0,
};
/** 不携带输入内容的历史摘要。 */
export const RECORDING_SUMMARY: RecordingSummary = {
  schema_version: 1, recording_id: RECORDING_FIXTURE.trace.recording_id,
  started_at_unix_ms: RECORDING_FIXTURE.trace.started_at_unix_ms, duration_ms: 100,
  raw_event_count: 1, semantic_record_count: 1, dropped_events: 0,
};
