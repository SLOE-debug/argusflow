import Pencil from 'lucide-react/dist/esm/icons/pencil.mjs';

import { Button } from '../../../ui';

/** 属性面板只提供进入独立 AQL 工作区的明确动作。 */
export function AqlEditButton({ onEdit }: Readonly<{ onEdit: () => void }>) {
  return (
    <Button
      icon={Pencil}
      className="w-full justify-center"
      onClick={onEdit}
    >
      编辑查找条件
    </Button>
  );
}
