import {
  Play,
  ShieldCheck,
  Upload,
} from 'lucide-react';

import { Button, SplitButton } from '../../../ui';

type EditorPrimaryActionsProps = Readonly<{
  /** 后端运行是否正在进行。 */
  running: boolean;
  /** 请求结构校验。 */
  onValidate: () => void;
  /** 请求运行当前工作流。 */
  onRun: () => void;
  /** 请求发布当前工作流。 */
  onPublish: () => void;
}>;

/** 标题栏中的校验、运行和发布主操作。 */
export function EditorPrimaryActions({
  running,
  onValidate,
  onRun,
  onPublish,
}: EditorPrimaryActionsProps) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <Button
        variant="secondary"
        size="compact"
        icon={ShieldCheck}
        className="hidden min-[1100px]:flex"
        onClick={onValidate}
        disabled={running}
        aria-label="检查工作流"
      >
        检查流程
      </Button>
      <SplitButton
        label={running ? '运行中…' : '运行'}
        icon={Play}
        disabled={running}
        onPrimaryClick={onRun}
      />
      <span className="hidden min-[1360px]:block">
        <SplitButton
          label="发布"
          icon={Upload}
          onPrimaryClick={onPublish}
        />
      </span>
    </div>
  );
}
