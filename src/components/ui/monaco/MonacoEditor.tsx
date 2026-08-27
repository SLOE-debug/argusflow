import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';
import type * as Monaco from 'monaco-editor/editor/editor.api';

import { acquireMonacoModel, releaseMonacoModel } from './modelRegistry';
import { loadMonacoEditor, type MonacoApi } from './monacoLoader';
import { MONACO_EDITOR_THEME } from './shellSyntaxHighlighting';

/** Monaco 完成挂载前允许业务语言注册自己的 provider。 */
export type MonacoConfigurator = (monaco: MonacoApi) => void | Promise<void>;

/** 供编辑器标题栏调用、同时保留 Monaco undo 栈的命令。 */
export type MonacoEditorHandle = Readonly<{
  /** 聚焦当前编辑器。 */
  focus: () => void;
  /** 运行 Monaco 当前语言注册的标准 Format Document 动作。 */
  formatDocument: () => Promise<void>;
}>;

export type MonacoEditorProps = Readonly<{
  /** 文档当前受控值。 */
  value: string;
  /** 用户编辑后的完整文档。 */
  onChange: (value: string) => void;
  /** Monaco 语言标识。 */
  language: string;
  /** 在应用生命周期内唯一且稳定的模型 URI。 */
  modelUri: string;
  /** Monaco 隐藏输入区的可访问名称。 */
  ariaLabel: string;
  /** 编辑器容器样式。 */
  className?: string;
  /** 在创建模型前注册业务语言能力。 */
  configure?: MonacoConfigurator;
  /** 业务编辑器覆盖项。 */
  options?: Readonly<Monaco.editor.IStandaloneEditorConstructionOptions>;
}>;

/** 通用受控 Monaco 编辑器；模型所有权由稳定 URI 管理。 */
export const MonacoEditor = forwardRef<MonacoEditorHandle, MonacoEditorProps>(
  function MonacoEditor({
    value,
    onChange,
    language,
    modelUri,
    ariaLabel,
    className = '',
    configure,
    options,
  }, forwardedRef) {
    const containerRef = useRef<HTMLDivElement>(null);
    const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
    const modelRef = useRef<Monaco.editor.ITextModel | null>(null);
    const monacoRef = useRef<MonacoApi | null>(null);
    const onChangeRef = useRef(onChange);
    const configureRef = useRef(configure);
    const valueRef = useRef(value);
    const optionsRef = useRef(options);
    const applyingControlledValueRef = useRef(false);
    const [loadError, setLoadError] = useState<string | null>(null);

    onChangeRef.current = onChange;
    configureRef.current = configure;
    valueRef.current = value;
    optionsRef.current = options;

    useImperativeHandle(forwardedRef, () => ({
      focus: () => editorRef.current?.focus(),
      formatDocument: async () => {
        await editorRef.current
          ?.getAction('editor.action.formatDocument')
          ?.run();
      },
    }), []);

    useEffect(() => {
      let active = true;
      let modelAcquired = false;
      let contentSubscription: Monaco.IDisposable | null = null;

      void loadMonacoEditor()
        .then(async (monaco) => {
          await configureRef.current?.(monaco);
          const container = containerRef.current;
          if (!active || !container) {
            return;
          }

          const model = acquireMonacoModel(monaco, modelUri, language, valueRef.current);
          modelAcquired = true;
          if (model.getValue() !== valueRef.current) {
            model.setValue(valueRef.current);
          }
          const editor = monaco.editor.create(container, {
            automaticLayout: true,
            ariaLabel,
            fontFamily: 'Cascadia Code, Consolas, monospace',
            fontSize: 12,
            fixedOverflowWidgets: true,
            lineHeight: 20,
            minimap: { enabled: false },
            overviewRulerLanes: 0,
            padding: { top: 8, bottom: 8 },
            renderLineHighlight: 'line',
            scrollBeyondLastLine: false,
            'semanticHighlighting.enabled': true,
            theme: MONACO_EDITOR_THEME,
            ...optionsRef.current,
            model,
          });

          monacoRef.current = monaco;
          modelRef.current = model;
          editorRef.current = editor;
          contentSubscription = model.onDidChangeContent(() => {
            if (!applyingControlledValueRef.current) {
              onChangeRef.current(model.getValue());
            }
          });
          setLoadError(null);
        })
        .catch(() => {
          if (active) {
            setLoadError('编辑器暂时无法加载，请稍后重试。');
          }
        });

      return () => {
        active = false;
        contentSubscription?.dispose();
        editorRef.current?.dispose();
        editorRef.current = null;
        modelRef.current = null;
        monacoRef.current = null;
        if (modelAcquired) {
          releaseMonacoModel(modelUri);
        }
      };
    }, [ariaLabel, language, modelUri]);

    useEffect(() => {
      const model = modelRef.current;
      if (!model || model.getValue() === value) {
        return;
      }
      applyingControlledValueRef.current = true;
      model.setValue(value);
      applyingControlledValueRef.current = false;
    }, [value]);

    useEffect(() => {
      if (options) {
        editorRef.current?.updateOptions(options);
      }
    }, [options]);

    useEffect(() => {
      const monaco = monacoRef.current;
      if (monaco) {
        void configure?.(monaco);
      }
    }, [configure]);

    return (
      <div
        className={`relative overflow-hidden rounded-md border border-slate-300 bg-white ${className}`}
      >
        <div ref={containerRef} className="absolute inset-0" />
        {loadError ? (
          <p
            className="absolute inset-0 flex items-center justify-center bg-amber-50 px-3 text-center text-[10px] text-amber-700"
            role="alert"
          >
            {loadError}
          </p>
        ) : null}
      </div>
    );
  },
);
