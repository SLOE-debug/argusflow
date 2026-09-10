import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { DraftController, downloadAql, exportSource, readAqlFile } from '../../features/aql';
import type { LanguageService } from '../../features/aql';
import { Button, FileInput } from '../ui';
import { CodeEditor } from './monaco/CodeEditor';

/** 首次进入时的完整可编辑示例。 */
const INITIAL_SOURCE = '窗口(名称 包含 "设置") >> 按钮(名称 = "保存", 可用 = 真)';
/** 编辑工作区通过领域控制器维护草稿，视图不解释 AQL。 */
export function EditorWorkspace({ service }: { readonly service: LanguageService }) {
  const [controller] = useState(() => new DraftController(INITIAL_SOURCE, async (source) => service.inspect(source)));
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot);
  const [message, setMessage] = useState('');
  const importVersion = useRef(0);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    void controller.refresh();
    return () => { active.current = false; importVersion.current++; };
  }, [controller]);
  const analysis = state.status === 'ready' ? state.analysis : undefined;
  const english = exportSource(state);
  const status = state.status === 'pending' ? '正在编辑，完成输入后检查' :
    state.status === 'failed' ? state.message :
    state.analysis.diagnostics.length ? `发现 ${state.analysis.diagnostics.length} 处问题` : '语法检查通过';
  const change = (source: string) => { importVersion.current++; controller.edit(source); };
  const importFile = async (file: File) => {
    const version = ++importVersion.current;
    try {
      const source = await readAqlFile(file, service);
      if (!active.current || version !== importVersion.current) return;
      controller.edit(source);
      setMessage('已导入。可使用编辑器撤销恢复之前的内容。');
    } catch (error) {
      if (active.current && version === importVersion.current) setMessage(error instanceof Error ? error.message : '导入失败，请重新选择文件。');
    }
  };
  return (
    <section className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-200 px-6 py-4">
        <div>
          <h2 className="font-semibold">查询编辑器</h2>
          <p className="mt-1 text-xs text-slate-500">中文编辑 · 英文导出</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <FileInput accept=".aql" label="选择 AQL 文件" buttonLabel="导入 .aql" onFile={(file) => void importFile(file)} />
          <Button
            disabled={analysis?.formatted === undefined}
            onClick={() => { if (analysis?.formatted !== undefined) change(analysis.formatted); }}
          >格式化</Button>
          <Button
            disabled={english === undefined}
            className="border-blue-700 bg-blue-700 text-white hover:bg-blue-800"
            onClick={() => { if (english !== undefined) downloadAql(english); }}
          >导出英文 AQL</Button>
        </div>
      </div>
      <div className="grid min-w-0 lg:grid-cols-2">
        <div className="min-w-0 border-b border-slate-200 lg:border-r lg:border-b-0">
          <div className="border-b border-slate-100 px-6 py-3 text-xs font-medium text-slate-500">中文查询</div>
          <CodeEditor
            source={state.source}
            service={service}
            diagnostics={analysis?.diagnostics ?? []}
            onChange={change}
            onComposition={(active) => controller.composition(active)}
            onError={() => setMessage('编辑器加载失败，请刷新页面重试。')}
          />
        </div>
        <div className="min-w-0 bg-slate-50/60">
          <div className="border-b border-slate-100 px-6 py-3 text-xs font-medium text-slate-500">导出内容预览</div>
          <pre
            aria-label="英文 AQL 预览"
            className="min-h-48 whitespace-pre-wrap break-words px-6 py-5 font-mono text-sm leading-7 text-slate-700"
          >{english ?? '完成查询并修正语法后，这里会显示英文 AQL。'}</pre>
          <div className="mx-6 rounded-lg border border-slate-200 bg-white p-4 text-xs leading-6 text-slate-500">
            <p>例如：按钮 → button，名称 → name。</p>
            <p>引号中的“保存”、参数名和正则内容保持原样。</p>
          </div>
        </div>
      </div>
      <footer className="border-t border-slate-200 px-6 py-4">
        <p role="status" className="text-sm text-slate-600">{status}</p>
        {analysis?.diagnostics.map((item, index) => (
          <p key={index} className="mt-2 text-sm text-red-700">
            第 {item.range.start.line + 1} 行，第 {item.range.start.column + 1} 列：{item.message}
          </p>
        ))}
        {message && <p role="alert" className="mt-2 text-sm text-slate-600">{message}</p>}
      </footer>
    </section>
  );
}
