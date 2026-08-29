import { useId } from 'react';

import {
  deriveFocusSelection,
  type FocusCandidate,
} from '../../../features/workflow';

type FocusMaskProps = Readonly<{
  /** Host 按 artifact_id 解析后的本地图片 URL。 */
  imageSource: string;
  /** artifact 元数据中的原始像素宽度。 */
  imageWidth: number;
  /** artifact 元数据中的原始像素高度。 */
  imageHeight: number;
  /** Runtime 已经过查询过滤和排序的合法候选。 */
  candidates: ReadonlyArray<FocusCandidate>;
  /** 是否同时展示 ambiguous 的全部候选。 */
  showAllCandidates?: boolean;
}>;

/** 在原始截图上渲染矢量解释层，不把遮罩烧入诊断 PNG。 */
export function FocusMask({
  imageSource,
  imageWidth,
  imageHeight,
  candidates,
  showAllCandidates = true,
}: FocusMaskProps) {
  const maskId = `focus-mask-${useId().replaceAll(':', '')}`;
  const selection = deriveFocusSelection(candidates);
  const visibleCandidates = selection.outcome === 'ambiguous' && !showAllCandidates
    ? selection.candidates.slice(0, 1)
    : selection.candidates;

  return (
    <figure className="min-w-0">
      <div
        className="relative w-full overflow-hidden rounded-md border border-slate-200 bg-slate-950"
        style={{ aspectRatio: `${imageWidth} / ${imageHeight}` }}
      >
        <img
          src={imageSource}
          alt="OCR 模型输入"
          className="absolute inset-0 size-full object-contain"
        />
        <svg
          viewBox={`0 0 ${imageWidth} ${imageHeight}`}
          className="absolute inset-0 size-full"
          role="img"
          aria-label={selectionLabel(selection.outcome, selection.candidates.length)}
        >
          {selection.outcome === 'unique' ? (
            <defs>
              <mask id={maskId}>
                <rect width={imageWidth} height={imageHeight} fill="white" />
                <polygon points={candidatePoints(selection.selected)} fill="black" />
              </mask>
            </defs>
          ) : null}
          <rect
            width={imageWidth}
            height={imageHeight}
            fill="rgb(15 23 42)"
            fillOpacity={selection.outcome === 'unique' ? 0.5 : 0.32}
            mask={selection.outcome === 'unique' ? `url(#${maskId})` : undefined}
          />
          {visibleCandidates.map((candidate, index) => (
            <g key={candidate.id}>
              <polygon
                points={candidatePoints(candidate)}
                fill={selection.outcome === 'unique' || index === 0 ? '#2563eb' : '#f59e0b'}
                fillOpacity={selection.outcome === 'unique' ? 0.14 : index === 0 ? 0.25 : 0.12}
                stroke={selection.outcome === 'unique' || index === 0 ? '#60a5fa' : '#fbbf24'}
                strokeWidth={Math.max(1, Math.min(imageWidth, imageHeight) / 300)}
                vectorEffect="non-scaling-stroke"
              />
              <text
                x={candidate.bbox.x}
                y={Math.max(12, candidate.bbox.y - 5)}
                fill="white"
                fontSize={Math.max(10, Math.min(imageWidth, imageHeight) / 35)}
                fontWeight="600"
                stroke="rgb(15 23 42)"
                strokeWidth="3"
                paintOrder="stroke"
              >
                {candidateLabel(selection.outcome, index, candidate.confidence)}
              </text>
            </g>
          ))}
        </svg>
      </div>
      <figcaption className="mt-1 flex items-center justify-between gap-3 text-[10px]">
        <span className={selection.outcome === 'ambiguous' ? 'font-semibold text-amber-700' : 'text-slate-500'}>
          {selectionLabel(selection.outcome, selection.candidates.length)}
        </span>
        <span className={selection.outcome === 'unique' ? 'text-emerald-700' : 'text-slate-500'}>
          {selection.outcome === 'unique' ? 'SendInput 可继续' : 'SendInput 未执行'}
        </span>
      </figcaption>
    </figure>
  );
}

function candidatePoints(candidate: FocusCandidate): string {
  const points = candidate.polygon.length >= 3
    ? candidate.polygon
    : [
        { x: candidate.bbox.x, y: candidate.bbox.y },
        { x: candidate.bbox.x + candidate.bbox.width, y: candidate.bbox.y },
        {
          x: candidate.bbox.x + candidate.bbox.width,
          y: candidate.bbox.y + candidate.bbox.height,
        },
        { x: candidate.bbox.x, y: candidate.bbox.y + candidate.bbox.height },
      ];
  return points.map((point) => `${point.x},${point.y}`).join(' ');
}

function candidateLabel(
  outcome: 'not_found' | 'unique' | 'ambiguous',
  index: number,
  confidence: number,
): string {
  const percent = Math.round(Math.max(0, Math.min(1, confidence)) * 100);
  return outcome === 'unique' ? `唯一命中 · ${percent}%` : `Candidate #${index + 1} · ${percent}%`;
}

function selectionLabel(
  outcome: 'not_found' | 'unique' | 'ambiguous',
  count: number,
): string {
  if (outcome === 'unique') return '唯一命中 / Selected Target';
  if (outcome === 'ambiguous') return `Ambiguous · ${count} 个合法候选，执行已阻止`;
  return '未找到合法候选';
}
