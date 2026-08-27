/** Vitest 的全局测试初始化：注册 jest-dom 扩展断言，使组件测试可验证 DOM 状态。 */
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import {
  createElement,
  forwardRef,
  useImperativeHandle,
} from 'react';
import type { ChangeEvent } from 'react';
import { afterEach, vi } from 'vitest';

/** jsdom 不提供 Monaco 所需的布局与 Worker；组件测试使用等价受控文本边界。 */
vi.mock('../components/ui/monaco', () => ({
  MonacoEditor: forwardRef(function MonacoEditorMock(props: Readonly<{
    ariaLabel: string;
    language: string;
    value: string;
    onChange: (value: string) => void;
  }>, ref) {
    useImperativeHandle(ref, () => ({
      focus: () => undefined,
      formatDocument: async () => undefined,
    }), []);
    return createElement('textarea', {
      'aria-label': props.ariaLabel,
      'data-language': props.language,
      value: props.value,
      onChange: (event: ChangeEvent<HTMLTextAreaElement>) => (
        props.onChange(event.target.value)
      ),
    });
  }),
}));

/** 每个测试结束后卸载 React 树，避免跨用例残留 DOM 与订阅。 */
afterEach(cleanup);
