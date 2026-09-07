/** 固定 CDP 检查函数的纯对象测试；不启动或控制浏览器。 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

/** 以固定源码构造被测 inspector，不能从用户输入拼接代码。 */
const inspect = new Function(`return (${readFileSync(new URL('../src/inspection/inspect.js', import.meta.url), 'utf8')});`)();
globalThis.Node = { ELEMENT_NODE: 1 };

/** 提供最小 DOM 事实；任何 value 读取都会使测试失败。 */
function element(attributes = {}, options = {}) {
    const view = {};
    view.top = view;
    const document = { defaultView: view, getElementById: () => null };
    const node = {
        nodeType: 1,
        localName: 'input',
        type: attributes.type || 'text',
        id: attributes.id || '',
        className: 'field',
        children: [],
        parentElement: null,
        isContentEditable: false,
        ownerDocument: document,
        getAttribute: key => attributes[key] ?? null,
        hasAttribute: key => key in attributes,
        matches: () => 'data-sensitive' in attributes || 'data-private' in attributes,
        closest: () => null,
        getRootNode: () => document,
        getBoundingClientRect: () => ({ x: 20, y: 30, width: 100, height: 20 }),
        ...options,
    };
    Object.defineProperty(node, 'value', { get: () => { throw new Error('inspector must not read input values'); } });
    return node;
}

test('password metadata is retained without reading or returning the field value', () => {
    const result = inspect.call(element({ type: 'password', id: 'password', 'aria-label': '密码', 'data-testid': 'login-password' }));
    assert.equal(result.sensitive, true);
    assert.equal(result.semantics.name, null);
    assert.equal(result.semantics.test_id, 'login-password');
    assert.equal(result.semantics.role, 'text_box');
    assert.equal(result.editable, true);
});

test('plain labels produce deterministic semantics and preserve CSS bounds', () => {
    const result = inspect.call(element({ id: 'name', 'data-testid': 'customer-name', 'aria-label': '姓名' }));
    assert.equal(result.sensitive, false);
    assert.equal(result.semantics.name, '姓名');
    assert.deepEqual(result.bounds, { x: 20, y: 30, width: 100, height: 20 });
});

test('sensitive autocomplete and explicit markers are recognized', () => {
    for (const attributes of [{ autocomplete: 'one-time-code' }, { autocomplete: 'cc-number' }, { 'data-sensitive': '' }]) {
        assert.equal(inspect.call(element(attributes)).sensitive, true);
    }
});

test('contenteditable values are never converted into accessible names', () => {
    const node = element({}, { localName: 'div', isContentEditable: true });
    Object.defineProperty(node, 'innerText', { get: () => { throw new Error('contenteditable text is a value'); } });
    Object.defineProperty(node, 'textContent', { get: () => { throw new Error('contenteditable text is a value'); } });
    assert.equal(inspect.call(node).semantics.name, null);
});

test('iframe and shadow roots do not claim replayable selectors', () => {
    const frame = element({}, { localName: 'iframe' });
    assert.equal(inspect.call(frame).replayable, false);
    const shadow = element({}, { getRootNode: () => ({}) });
    assert.equal(inspect.call(shadow).replayable, false);
});
