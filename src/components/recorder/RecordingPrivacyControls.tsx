import { Button } from '../ui';

/** 分享前提供隐私检查入口，避免要求业务用户理解录制协议。 */
export function RecordingPrivacyControls({ onReview }: Readonly<{ onReview: () => void }>) {
  return (
    <div className="flex flex-wrap items-center gap-3 border-b border-blue-100 bg-blue-50/60 px-3 py-2">
      <p className="flex-1 text-xs leading-5 text-slate-600">有不想分享的内容？可以先检查隐私，删除私密内容或给截图打马赛克。</p>
      <Button
        size="compact"
        onClick={onReview}
      >检查隐私内容</Button>
    </div>
  );
}
