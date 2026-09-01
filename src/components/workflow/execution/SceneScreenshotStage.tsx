import ImageOff from 'lucide-react/dist/esm/icons/image-off.mjs';
import { useEffect, useMemo, useState } from 'react';

import {
  readRunArtifact,
  type RunArtifactKind,
  type RunArtifactSummary,
  type SceneNodeRef,
  type VisualQueryTrace,
} from '../../../features/workflow';
import { Button, Select } from '../../ui';

type SceneScreenshotStageProps = Readonly<{
  runId: string;
  trace: VisualQueryTrace;
  artifacts: ReadonlyArray<RunArtifactSummary>;
}>;

const LAYERS = [
  { kind: 'captured_frame', label: '捕获帧' },
  { kind: 'ocr_source_roi', label: 'OCR 区域' },
  { kind: 'ocr_model_input', label: '模型输入' },
] as const satisfies ReadonlyArray<{ kind: RunArtifactKind; label: string }>;

/** 按窗口与帧复合身份选择图像；只有捕获帧叠加同坐标系 OCR 标注。 */
export function SceneScreenshotStage({ runId, trace, artifacts }: SceneScreenshotStageProps) {
  const [windowHandle, setWindowHandle] = useState(trace.projection.windows[0]?.window_handle ?? '');
  const [layer, setLayer] = useState<RunArtifactKind>('captured_frame');
  const windowProjection = trace.projection.windows.find((window) => (
    window.window_handle === windowHandle
  )) ?? trace.projection.windows[0] ?? null;
  const relevantArtifacts = useMemo(() => artifacts.filter((artifact) => (
    artifact.window_handle === windowProjection?.window_handle
    && artifact.frame_id === windowProjection.frame_id
    && (artifact.kind === 'captured_frame' || artifact.node_sequence === trace.node_sequence)
  )), [artifacts, trace.node_sequence, windowProjection]);
  const selectedArtifact = relevantArtifacts.find((artifact) => artifact.kind === layer) ?? null;
  const [source, setSource] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedArtifact) {
      setSource(null);
      return undefined;
    }
    let disposed = false;
    let objectUrl: string | null = null;
    setLoadError(null);
    setSource(null);
    void readRunArtifact(runId, selectedArtifact.artifact_id).then((body) => {
      if (disposed) return;
      objectUrl = URL.createObjectURL(new Blob([body], { type: selectedArtifact.mime_type }));
      setSource(objectUrl);
    }).catch((cause) => {
      if (!disposed) setLoadError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [runId, selectedArtifact]);

  if (!windowProjection) return <ScreenshotEmpty message="本次查询没有可选择的窗口。" />;
  return (
    <div className="grid h-full min-h-0 grid-rows-[52px_minmax(0,1fr)] bg-slate-950">
      <header className="flex items-center gap-3 border-b border-slate-700 bg-slate-900 px-4">
        <Select
          aria-label="截图窗口"
          density="compact"
          value={windowProjection.window_handle}
          containerClassName="w-52"
          options={trace.projection.windows.map((window) => ({
            value: window.window_handle,
            label: `窗口 ${window.window_handle} · 帧 ${window.frame_id}`,
          }))}
          onValueChange={setWindowHandle}
        />
        <div className="flex items-center gap-1">
          {LAYERS.map((candidate) => (
            <Button
              key={candidate.kind}
              size="compact"
              variant={layer === candidate.kind ? 'secondary' : 'ghost'}
              disabled={!relevantArtifacts.some((artifact) => artifact.kind === candidate.kind)}
              onClick={() => setLayer(candidate.kind)}
            >
              {candidate.label}
            </Button>
          ))}
        </div>
      </header>
      <div className="flex min-h-0 items-center justify-center overflow-auto p-5">
        {loadError ? (
          <ScreenshotEmpty message={`图像读取失败：${loadError}`} />
        ) : !selectedArtifact ? (
          <ScreenshotEmpty message="这一窗口和帧没有保存所选图像，请重新运行后再查看。" />
        ) : source ? (
          layer === 'captured_frame' ? (
            <AnnotatedFrame
              source={source}
              artifact={selectedArtifact}
              trace={trace}
              windowHandle={windowProjection.window_handle}
            />
          ) : (
            <img src={source} alt={LAYERS.find((item) => item.kind === layer)?.label} className="max-h-full max-w-full object-contain" />
          )
        ) : <span className="text-[13px] text-slate-400">正在读取图像…</span>}
      </div>
    </div>
  );
}

function AnnotatedFrame({
  source,
  artifact,
  trace,
  windowHandle,
}: Readonly<{
  source: string;
  artifact: RunArtifactSummary;
  trace: VisualQueryTrace;
  windowHandle: string;
}>) {
  const width = artifact.width ?? 1;
  const height = artifact.height ?? 1;
  const candidateKeys = new Set(trace.candidate_nodes.map(nodeRefKey));
  const selectedKey = trace.selected_node ? nodeRefKey(trace.selected_node) : null;
  return (
    <svg className="max-h-full max-w-full" viewBox={`0 0 ${width} ${height}`}>
      <image href={source} width={width} height={height} />
      {trace.projection.nodes.filter((node) => node.window_handle === windowHandle).map((node) => {
        const key = nodeRefKey(node);
        const selected = key === selectedKey;
        const candidate = candidateKeys.has(key);
        return (
          <rect
            key={key}
            x={node.frame_bbox.x}
            y={node.frame_bbox.y}
            width={node.frame_bbox.width}
            height={node.frame_bbox.height}
            fill={selected ? '#10b98126' : candidate ? '#f59e0b20' : 'transparent'}
            stroke={selected ? '#34d399' : candidate ? '#fbbf24' : '#60a5fa'}
            strokeOpacity={selected || candidate ? 1 : 0.25}
            strokeWidth={selected ? 3 : candidate ? 2 : 1}
            vectorEffect="non-scaling-stroke"
          />
        );
      })}
    </svg>
  );
}

function nodeRefKey(node: SceneNodeRef): string {
  return `${node.window_handle}:${node.scene_id}:${node.node_id}`;
}

function ScreenshotEmpty({ message }: Readonly<{ message: string }>) {
  return (
    <div className="flex h-full w-full items-center justify-center text-center">
      <div>
        <ImageOff className="mx-auto size-8 text-slate-600" aria-hidden="true" />
        <p className="mt-2 text-[13px] text-slate-400">{message}</p>
      </div>
    </div>
  );
}
