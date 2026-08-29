import { useEffect, useMemo, useState } from 'react';

import {
  readRunArtifact,
  type RunArtifactKind,
  type RunArtifactSummary,
  type VisualQueryTrace,
} from '../../../features/workflow';
import { Button } from '../../ui';
import { FocusMask } from './FocusMask';

type OcrArtifactViewerProps = Readonly<{
  runId: string;
  artifacts: ReadonlyArray<RunArtifactSummary>;
  queryTrace?: VisualQueryTrace | null;
}>;

/** 三层 OCR 图像查看器；默认打开真正的模型输入。 */
export function OcrArtifactViewer({ runId, artifacts, queryTrace = null }: OcrArtifactViewerProps) {
  const requestId = artifacts.find((artifact) => artifact.kind === 'ocr_model_input')?.request_id
    ?? artifacts.find((artifact) => artifact.kind === 'ocr_source_roi')?.request_id
    ?? null;
  const requestArtifacts = useMemo(() => artifacts.filter((artifact) => (
    artifact.request_id === requestId
    || (artifact.kind === 'captured_frame' && artifact.frame_id === artifacts.find(
      (item) => item.request_id === requestId,
    )?.frame_id)
  )), [artifacts, requestId]);
  const defaultArtifact = requestArtifacts.find((artifact) => artifact.kind === 'ocr_model_input')
    ?? requestArtifacts.find((artifact) => artifact.kind === 'ocr_source_roi')
    ?? requestArtifacts[0]
    ?? null;
  const [selectedArtifactId, setSelectedArtifactId] = useState<string | null>(
    defaultArtifact?.artifact_id ?? null,
  );
  const [source, setSource] = useState<string | null>(null);
  const selectedArtifact = requestArtifacts.find(
    (artifact) => artifact.artifact_id === selectedArtifactId,
  ) ?? defaultArtifact;

  useEffect(() => {
    setSelectedArtifactId(defaultArtifact?.artifact_id ?? null);
  }, [defaultArtifact?.artifact_id]);

  useEffect(() => {
    if (!selectedArtifact) return undefined;
    setSource(null);
    let disposed = false;
    let objectUrl: string | null = null;
    void readRunArtifact(runId, selectedArtifact.artifact_id).then((body) => {
      if (disposed) return;
      objectUrl = URL.createObjectURL(new Blob([body], { type: selectedArtifact.mime_type }));
      setSource(objectUrl);
    });
    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [runId, selectedArtifact]);

  if (!selectedArtifact) return null;
  return (
    <section className="mb-2 rounded-md border border-slate-200 p-2">
      <div className="mb-2 flex items-center gap-1">
        {ARTIFACT_TABS.map((tab) => {
          const artifact = requestArtifacts.find((item) => item.kind === tab.kind);
          return (
            <Button
              key={tab.kind}
              variant={artifact?.artifact_id === selectedArtifact.artifact_id ? 'secondary' : 'ghost'}
              size="compact"
              disabled={!artifact}
              onClick={() => artifact && setSelectedArtifactId(artifact.artifact_id)}
            >
              {tab.label}
            </Button>
          );
        })}
      </div>
      <div className="flex min-h-32 items-center justify-center overflow-hidden rounded bg-slate-950">
        {source && selectedArtifact.kind === 'captured_frame' && queryTrace
          && queryTrace.candidates.length > 0
          && ['unique', 'ambiguous'].includes(queryTrace.outcome) ? (
          <FocusMask
            imageSource={source}
            imageWidth={selectedArtifact.width ?? 1}
            imageHeight={selectedArtifact.height ?? 1}
            candidates={queryTrace.candidates.map((candidate, index) => ({
              id: `${queryTrace.scene_id}-${index}`,
              rawText: candidate.raw_text,
              confidence: candidate.confidence,
              polygon: [],
              bbox: candidate.bbox,
            }))}
          />
        ) : source ? (
          <img
            src={source}
            alt={ARTIFACT_TABS.find((tab) => tab.kind === selectedArtifact.kind)?.label}
            className="max-h-72 max-w-full object-contain"
          />
        ) : (
          <span className="text-[10px] text-slate-400">正在读取诊断图像…</span>
        )}
      </div>
      <p className="mt-1 font-mono text-[9px] text-slate-400">
        {selectedArtifact.width ?? '?'} × {selectedArtifact.height ?? '?'} · Frame {selectedArtifact.frame_id}
      </p>
    </section>
  );
}

const ARTIFACT_TABS = [
  { kind: 'captured_frame', label: '捕获帧' },
  { kind: 'ocr_source_roi', label: 'OCR ROI' },
  { kind: 'ocr_model_input', label: '模型输入' },
] as const satisfies ReadonlyArray<Readonly<{ kind: RunArtifactKind; label: string }>>;
