/** Vitest 的全局测试初始化：注册 jest-dom 扩展断言，使组件测试可验证 DOM 状态。 */
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

/** 每个测试结束后卸载 React 树，避免跨用例残留 DOM 与订阅。 */
afterEach(cleanup);
