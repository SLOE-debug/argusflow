import { useEffect, useState } from 'react';
import { loadLanguageService } from '../../features/aql';
import type { LanguageService } from '../../features/aql';
import { EditorWorkspace } from './EditorWorkspace';

/** 页面入口只加载语言服务并装配编辑工作区。 */
export function EditorPage() {
  const [service, setService] = useState<LanguageService>();
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    void loadLanguageService().then((service) => {
      if (active) setService(service);
    }).catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, []);
  return (
    <main className="min-h-screen bg-slate-50 px-6 py-10 text-slate-900 sm:px-10">
      <div className="mx-auto max-w-7xl">
        <header className="mb-8">
          <p className="mb-3 text-xs font-semibold tracking-[0.2em] text-blue-700">ARGUSFLOW / AQL</p>
          <h1 className="text-3xl font-semibold tracking-tight">用中文编写查询</h1>
          <p className="mt-3 max-w-2xl text-sm leading-7 text-slate-600">
            角色和条件使用中文关键字，字符串保持原样。导出为英文 AQL，供定位 API 使用。
          </p>
        </header>
        {service
          ? <EditorWorkspace service={service} />
          : <p role={failed ? 'alert' : 'status'}>{failed ? '语言服务加载失败，请刷新页面重试。' : '正在准备编辑器…'}</p>}
      </div>
    </main>
  );
}
