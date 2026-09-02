import Braces from 'lucide-react/dist/esm/icons/braces.mjs';

import { Button } from '../../../ui';

/** 以次要代码入口打开独立 AQL 编辑器，不与主表单操作争夺视觉层级。 */
export function AqlEditButton({ onEdit }: Readonly<{ onEdit: () => void }>) {
  return (
    <Button
      icon={Braces}
      variant="ghost"
      size="compact"
      className="px-1.5 !text-blue-600 hover:!bg-blue-50 hover:!text-blue-700"
      onClick={onEdit}
    >
      编辑 AQL 查询
    </Button>
  );
}
