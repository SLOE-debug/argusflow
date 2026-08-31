import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';
import X from 'lucide-react/dist/esm/icons/x.mjs';
import { useEffect, useState } from 'react';

import type {
  ExecutionEvent,
  FlowComponentDefinition,
  JsonValue,
} from '../../../features/workflow';
import {
  findFlowComponent,
  isJsonObject,
  type FlowComponentCatalogItem,
} from '../../../features/workflow';

type ComponentDrillDownProps = Readonly<{
  definition: FlowComponentDefinition;
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>;
  events: ReadonlyArray<ExecutionEvent>;
  onClose: () => void;
}>;

type DrillDownLevel = Readonly<{
  definition: FlowComponentDefinition;
  /** 当前定义在上一层中对应的组件实例节点；根层为空。 */
  viaNodeId: string | null;
}>;

/** 在主画布上方展示锁定版本组件的内部可审计流程。 */
export function ComponentDrillDown({
  definition,
  componentCatalog,
  events,
  onClose,
}: ComponentDrillDownProps) {
  /** 从主画布实例到当前嵌套组件的精确版本路径。 */
  const [componentPath, setComponentPath] = useState<DrillDownLevel[]>([{
    definition,
    viaNodeId: null,
  }]);
  useEffect(() => setComponentPath([{ definition, viaNodeId: null }]), [definition]);
  const activeDefinition = componentPath.at(-1)?.definition ?? definition;
  const minX = Math.min(...activeDefinition.nodes.map((node) => node.position.x));
  const minY = Math.min(...activeDefinition.nodes.map((node) => node.position.y));
  return (
    <section className="absolute inset-0 z-40 flex min-h-0 flex-col bg-white">
      <header className="flex h-10 shrink-0 items-center border-b border-slate-200 bg-slate-50 px-3">
        <button
          type="button"
          className="text-[11px] font-medium text-blue-700 hover:text-blue-900"
          onClick={onClose}
        >
          主流程
        </button>
        {componentPath.map((level, index) => (
          <span
            key={`${level.definition.id}@${level.definition.version}:${level.viaNodeId ?? 'root'}`}
            className="flex items-center"
          >
            <ChevronRight className="mx-1.5 size-3 shrink-0 text-slate-400" aria-hidden="true" />
            {index === componentPath.length - 1 ? (
              <strong className="text-[11px] text-slate-800">{level.definition.name}</strong>
            ) : (
              <button
                type="button"
                className="text-[11px] font-medium text-blue-700 hover:text-blue-900"
                onClick={() => setComponentPath((current) => current.slice(0, index + 1))}
              >
                {level.definition.name}
              </button>
            )}
            <span className="ml-2 rounded bg-violet-100 px-1.5 py-0.5 font-mono text-[9px] text-violet-700">
              {level.definition.version}
            </span>
          </span>
        ))}
        <button
          type="button"
          aria-label="返回主流程"
          className="ml-auto flex size-7 items-center justify-center rounded text-slate-500 hover:bg-slate-200"
          onClick={onClose}
        >
          <X className="size-4 shrink-0" aria-hidden="true" />
        </button>
      </header>
      <div className="relative min-h-0 flex-1 overflow-auto bg-[radial-gradient(#dbe3ee_1px,transparent_1px)] bg-[size:24px_24px]">
        <svg className="pointer-events-none absolute inset-0 size-full overflow-visible">
          {activeDefinition.edges.map((edge) => {
            const source = activeDefinition.nodes.find((node) => node.id === edge.source);
            const target = activeDefinition.nodes.find((node) => node.id === edge.target);
            if (!source || !target) return null;
            return (
              <line
                key={edge.id}
                x1={source.position.x - minX + 192}
                y1={source.position.y - minY + 126}
                x2={target.position.x - minX + 48}
                y2={target.position.y - minY + 126}
                stroke="#7c3aed"
                strokeWidth="2"
              />
            );
          })}
        </svg>
        {activeDefinition.nodes.map((node) => {
          const runState = resolveInternalRunState(events, componentPath, node.id);
          return (
            <article
              key={node.id}
              className={
                'absolute flex h-[52px] w-[144px] flex-col justify-center rounded-lg border ' +
                `${runStateClasses(runState)} px-3 shadow-sm ` +
                (node.type_id === 'argus.component' ? 'cursor-zoom-in hover:border-violet-500' : '')
              }
              style={{
                left: node.position.x - minX + 48,
                top: node.position.y - minY + 100,
              }}
              onDoubleClick={() => {
                const reference = readComponentReference(node.payload);
                if (!reference) return;
                const nested = findFlowComponent(
                  reference.componentId,
                  reference.componentVersion,
                  componentCatalog,
                );
                if (nested) {
                  setComponentPath((current) => [...current, {
                    definition: nested.definition,
                    viaNodeId: node.id,
                  }]);
                }
              }}
            >
              <strong className="truncate text-[11px] text-slate-800">
                {componentNodeLabel(node.type_id)}
              </strong>
              <span className="truncate font-mono text-[9px] text-slate-400">{node.id}</span>
            </article>
          );
        })}
        <aside className="absolute right-4 top-4 w-52 rounded-lg border border-slate-200 bg-white/95 p-3 shadow-sm">
          <strong className="text-[10px] text-slate-700">组合步骤</strong>
          <p className="mt-2 text-[9px] font-semibold text-slate-500">输入</p>
          <p className="mt-0.5 font-mono text-[9px] text-slate-600">
            {activeDefinition.inputs.map((input) => input.key).join(', ') || '暂无'}
          </p>
          <p className="mt-2 text-[9px] font-semibold text-slate-500">输出</p>
          <p className="mt-0.5 font-mono text-[9px] text-slate-600">
            {activeDefinition.outputs.map((output) => output.name).join(', ') || '暂无'}
          </p>
        </aside>
        <p className="absolute bottom-4 left-4 rounded bg-white/90 px-2 py-1 text-[10px] text-slate-500 shadow-sm">
          这里仅供查看；要修改输入，请返回主流程的属性面板。
        </p>
      </div>
    </section>
  );
}

/** 从开放 JSON payload 读取版本锁定组件引用。 */
function readComponentReference(payload: JsonValue) {
  if (!isJsonObject(payload)) return null;
  const componentId = payload.component_id;
  const componentVersion = payload.component_version;
  return typeof componentId === 'string' && typeof componentVersion === 'string'
    ? { componentId, componentVersion }
    : null;
}

/** 根据 SourceMap 还原的组件路径查找当前内部节点最后一次执行状态。 */
function resolveInternalRunState(
  events: ReadonlyArray<ExecutionEvent>,
  componentPath: ReadonlyArray<DrillDownLevel>,
  nodeId: string,
): 'idle' | 'running' | 'success' | 'error' {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    const sourcePath = event.component_path;
    if (!sourcePath || sourcePath.length < componentPath.length) continue;
    const pathMatches = componentPath.every((level, depth) => {
      const frame = sourcePath[depth];
      const parentFrame = depth > 0 ? sourcePath[depth - 1] : null;
      return frame.component_id === level.definition.id
        && frame.component_version === level.definition.version
        && (level.viaNodeId === null || parentFrame?.inner_node_id === level.viaNodeId);
    });
    if (!pathMatches || sourcePath[componentPath.length - 1].inner_node_id !== nodeId) continue;
    if (event.kind === 'node_failed') return 'error';
    if (event.kind === 'node_succeeded') return 'success';
    if (event.kind === 'node_started') return 'running';
  }
  return 'idle';
}

/** 将内部节点执行状态映射为可审计的卡片颜色。 */
function runStateClasses(state: 'idle' | 'running' | 'success' | 'error'): string {
  switch (state) {
    case 'running':
      return 'border-blue-400 bg-blue-50';
    case 'success':
      return 'border-emerald-400 bg-emerald-50';
    case 'error':
      return 'border-red-400 bg-red-50';
    case 'idle':
      return 'border-violet-200 bg-white';
  }
}

/** 将内部开放节点 type_id 转换为紧凑中文标签。 */
function componentNodeLabel(typeId: string): string {
  const labels: Readonly<Record<string, string>> = {
    'argus.start': '开始',
    'argus.end': '结束',
    'argus.application': '打开应用',
    'argus.browser': '打开浏览器',
    'argus.browser.operation': '打开网页',
    'argus.ui': '操作界面',
    'argus.observe': '检查界面',
    'argus.loop': '重复执行',
    'argus.delay': '等待一段时间',
    'argus.fail': '停止并报错',
    'argus.data.format': '整理文本',
    'argus.component': '组合步骤',
  };
  return labels[typeId] ?? '自定义步骤';
}
