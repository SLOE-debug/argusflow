import {
  evidenceFor,
  reviewTitle,
  type RecordingRecord,
} from "../../features/recorder";
import { VideoReview } from "./VideoReview";
import { StructureDetails } from "./StructureDetails";
import {
  inputKeyLabel,
  resultFrameQuery,
} from "../../features/recorder/video-selection";
import { clickPoint } from "../../features/recorder/video-target";
import type { VideoFrame, VideoSeek } from "../../features/recorder/video";

/** 画面与结构详情共同展示当前时间线操作。 */
export function EvidenceDetails({
  action,
  records,
  directory,
  frequency,
  seek,
  onFrame,
  onReadError,
  onManualSeek,
}: {
  readonly action: RecordingRecord;
  readonly records: readonly RecordingRecord[];
  readonly directory: string;
  readonly frequency: number;
  readonly seek?: VideoSeek;
  readonly onFrame?: (frame: VideoFrame, requestSerial?: number) => void;
  readonly onReadError?: (message: string, requestSerial?: number) => void;
  readonly onManualSeek?: () => void;
}) {
  const evidence = evidenceFor(records, action);
  return (
    <section
      aria-label="操作回看"
      className="flex min-h-0 min-w-0 flex-1 flex-col"
    >
      <div className="flex min-h-0 flex-1">
        <VideoReview
          key={`${directory}:${action.id}`}
          directory={directory}
          action={action}
          point={clickPoint(action, records)}
          resultQuery={resultFrameQuery(action, records, frequency)}
          seek={seek}
          onFrame={onFrame}
          onReadError={onReadError}
          onManualSeek={onManualSeek}
        />
        <StructureDetails
          records={evidence}
          heading={
            <header className="mb-3 flex flex-wrap items-center gap-2 border-b border-line pb-3">
              <h2 className="font-semibold text-ink">{reviewTitle(action)}</h2>
              <span className="text-muted">
                {inputKeyLabel(action, records)}
              </span>
            </header>
          }
        />
      </div>
    </section>
  );
}
