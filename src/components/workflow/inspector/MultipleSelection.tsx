import { useState } from 'react';

import { Button, Input } from '../../ui';
import { InspectorField, InspectorSection } from './InspectorControls';

/** 展示多节点选择时可执行的组合步骤操作。 */
export function MultipleSelection({
  count,
  onCreateComponent,
}: Readonly<{
  count: number;
  onCreateComponent: (name: string, version: string) => boolean;
}>) {
  const [name, setName] = useState('新的组合步骤');
  const [version, setVersion] = useState('1.0.0');
  return (
    <InspectorSection title="已选择多个节点" last>
      <div className="rounded-md border border-dashed border-slate-300 px-3 py-5 text-center text-slate-600">
        <strong className="text-[13px]">已选择 {count} 个节点</strong>
        <p className="mt-1 text-[11px]">选择一段连续流程，即可保存并重复使用。</p>
      </div>
      <InspectorField label="组合步骤名称">
        <Input
          aria-label="组合步骤名称"
          value={name}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => setName(event.target.value)}
        />
      </InspectorField>
      <InspectorField label="初始版本">
        <Input
          aria-label="初始版本"
          value={version}
          className="font-mono"
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => setVersion(event.target.value)}
        />
      </InspectorField>
      <Button
        variant="primary"
        className="w-full"
        onClick={() => onCreateComponent(name, version)}
      >
        保存组合步骤
      </Button>
    </InspectorSection>
  );
}
