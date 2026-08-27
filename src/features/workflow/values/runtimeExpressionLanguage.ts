import type { MonacoApi } from '../../../components/ui/monaco';
import type { WorkflowCanvasNode } from '../model/workflowModel';
import { getNodeValueOutputs } from '../model/workflowNodeDefinitions';

/** 受限 Rhai 表达式在 Monaco 中使用的稳定语言 ID。 */
export const RUNTIME_EXPRESSION_LANGUAGE_ID = 'argusflow-runtime-expression';

type ExpressionSuggestion = Readonly<{
  /** 补全列表中展示的短名称。 */
  label: string;
  /** 写入文档的完整 Rhai 片段。 */
  insertText: string;
  /** 补全项右侧的作用说明。 */
  detail: string;
}>;

type ExpressionHoverTarget = Readonly<{
  /** 当前行必须同时包含的节点键，防止通用输出名匹配到其它节点。 */
  nodeKey: string;
  /** 鼠标所在单词对应的节点 ID 或输出名。 */
  token: string;
  /** 节点 ID 位于 nodeKey 内，输出名必须在 nodeKey 之后查找。 */
  type: 'node' | 'output';
  /** Hover 展示的工作流中文上下文。 */
  detail: string;
}>;

/** 每个 Monaco 文档独立保存当前工作流提供的节点与输出补全。 */
const documentSuggestions = new Map<string, ReadonlyArray<ExpressionSuggestion>>();

/** 每个表达式文档对应的节点与输出 Hover 上下文。 */
const documentHoverTargets = new Map<string, ReadonlyArray<ExpressionHoverTarget>>();

/** 同一个 Monaco 实例只注册一次语言和 provider。 */
const configuredMonacoInstances = new WeakSet<object>();

const BASE_SUGGESTIONS = [
  { label: 'input', insertText: 'input', detail: '流程输入（只读）' },
  { label: 'vars', insertText: 'vars', detail: '变量（只读）' },
  { label: 'nodes', insertText: 'nodes', detail: '节点输出（只读）' },
  { label: 'result', insertText: 'result', detail: '当前节点结果（仅用于输出）' },
  { label: 'str(value)', insertText: 'str(${1:value})', detail: '转换为文本' },
  { label: 'json(value)', insertText: 'json(${1:value})', detail: '转换为 JSON 文本' },
  {
    label: 'get(value, pointer)',
    insertText: 'get(${1:value}, ${2:"/path"})',
    detail: '按数据路径读取值',
  },
] as const satisfies ReadonlyArray<ExpressionSuggestion>;

/** 刷新一个表达式文档可见的节点 ID 与 Published Outputs 补全。 */
export function setRuntimeExpressionSuggestions(
  modelUri: string,
  nodes: ReadonlyArray<WorkflowCanvasNode>,
): void {
  const nodeSuggestions = nodes.flatMap((node): ExpressionSuggestion[] => {
    /** JSON.stringify 生成可直接粘贴到 Rhai 下标表达式中的安全字符串。 */
    const nodeKey = JSON.stringify(node.id);
    const nodeRoot = `nodes[${nodeKey}]`;
    return [
      {
        label: nodeRoot,
        insertText: nodeRoot,
        detail: `${node.data.label} 的全部 Published Outputs`,
      },
      ...getNodeValueOutputs(node.data).map((output) => ({
        label: `${node.id}.${output.name}`,
        insertText: `${nodeRoot}[${JSON.stringify(output.name)}]`,
        detail: `${node.data.label} · ${output.label}`,
      })),
    ];
  });
  documentSuggestions.set(modelUri, [...BASE_SUGGESTIONS, ...nodeSuggestions]);
  documentHoverTargets.set(modelUri, nodes.flatMap((node) => {
    const nodeKey = JSON.stringify(node.id);
    return [
      { nodeKey, token: node.id, type: 'node' as const, detail: node.data.label },
      ...getNodeValueOutputs(node.data).map((output) => ({
        nodeKey,
        token: output.name,
        type: 'output' as const,
        detail: `${node.data.label} · ${output.label}`,
      })),
    ];
  }));
}

/** 注册受限 Rhai 的基础高亮与上下文补全。 */
export function configureRuntimeExpressionLanguage(monaco: MonacoApi): void {
  if (configuredMonacoInstances.has(monaco)) return;
  configuredMonacoInstances.add(monaco);

  monaco.languages.register({ id: RUNTIME_EXPRESSION_LANGUAGE_ID });
  monaco.languages.setMonarchTokensProvider(RUNTIME_EXPRESSION_LANGUAGE_ID, {
    keywords: ['true', 'false', 'null', 'if', 'else'],
    tokenizer: {
      root: [
        [/[a-zA-Z_][\w]*/, { cases: { '@keywords': 'keyword', '@default': 'identifier' } }],
        [/-?\d+(?:\.\d+)?/, 'number'],
        [/"(?:[^"\\]|\\.)*"/, 'string'],
        [/[{}()[\]]/, '@brackets'],
        [/[+\-*\/%=!<>|&]+/, 'operator'],
        [/[;,\.]/, 'delimiter'],
      ],
    },
  });
  monaco.languages.registerCompletionItemProvider(RUNTIME_EXPRESSION_LANGUAGE_ID, {
    triggerCharacters: ['.', '['],
    provideCompletionItems: (model, position) => {
      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(
        position.lineNumber,
        word.startColumn,
        position.lineNumber,
        word.endColumn,
      );
      const suggestions = documentSuggestions.get(model.uri.toString()) ?? BASE_SUGGESTIONS;
      return {
        suggestions: suggestions.map((suggestion) => ({
          label: suggestion.label,
          kind: monaco.languages.CompletionItemKind.Value,
          insertText: suggestion.insertText,
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: suggestion.detail,
          range,
        })),
      };
    },
  });
  monaco.languages.registerHoverProvider(RUNTIME_EXPRESSION_LANGUAGE_ID, {
    provideHover: (model, position) => {
      const line = model.getLineContent(position.lineNumber);
      const column = position.column - 1;
      const target = (documentHoverTargets.get(model.uri.toString()) ?? []).find((candidate) => {
        if (!line.includes(candidate.nodeKey)) return false;
        const nodeKeyStart = line.indexOf(candidate.nodeKey);
        const tokenStart = line.indexOf(
          candidate.token,
          candidate.type === 'node' ? nodeKeyStart : nodeKeyStart + candidate.nodeKey.length,
        );
        return tokenStart >= 0
          && column >= tokenStart
          && column <= tokenStart + candidate.token.length;
      });
      return target ? { contents: [{ value: escapeMarkdown(target.detail) }] } : null;
    },
  });
}

/** Monaco Hover 使用 Markdown，必须转义工作流显示名中的控制字符。 */
function escapeMarkdown(value: string): string {
  return value.replace(/[\\`*_{}[\]()#+\-.!]/g, '\\$&');
}
