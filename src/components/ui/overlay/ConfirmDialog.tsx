import { Button } from '../button/Button';
import { Dialog } from './Dialog';

export type ConfirmDialogProps = Readonly<{
  /** 当前是否打开。 */
  open: boolean;
  /** 打开状态变化回调。 */
  onOpenChange: (open: boolean) => void;
  /** 确认框标题。 */
  title: string;
  /** 确认框说明。 */
  description: string;
  /** 确认按钮文字。 */
  confirmText?: string;
  /** 取消按钮文字。 */
  cancelText?: string;
  /** 确认操作回调。 */
  onConfirm: () => void;
  /** 确认操作是否暂时不可用。 */
  confirmDisabled?: boolean;
  /** 确认操作是否正在执行。 */
  loading?: boolean;
}>;

/** 由 Dialog 与 Button 组合而成的统一危险操作确认框。 */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmText = '确认',
  cancelText = '取消',
  onConfirm,
  confirmDisabled = false,
  loading = false,
}: ConfirmDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={description}
      footer={(
        <>
          <Button
            variant="secondary"
            onClick={() => onOpenChange(false)}
          >
            {cancelText}
          </Button>
          <Button
            variant="danger"
            loading={loading}
            disabled={confirmDisabled}
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
          >
            {confirmText}
          </Button>
        </>
      )}
    />
  );
}
