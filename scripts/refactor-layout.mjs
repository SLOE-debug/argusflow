/**
 * 将初版平铺源码迁移到职责目录，并把测试集中到仓库根 tests/。
 * 在仓库根运行 node scripts/refactor-layout.mjs；显式映射可重复执行。
 * 已迁移的目标不会覆盖，源/目标冲突会停止；不改变运行时协议和算法。
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
function resolve(relative) {
  const absolute = path.resolve(root, relative);
  if (!absolute.startsWith(root + path.sep)) throw new Error(`越出仓库：${relative}`);
  return absolute;
}
function move(source, destination) {
  const from = resolve(source);
  const to = resolve(destination);
  if (!fs.existsSync(from)) {
    if (!fs.existsSync(to)) throw new Error(`源和目标均不存在：${source}`);
    return;
  }
  if (fs.existsSync(to)) throw new Error(`拒绝覆盖迁移目标：${destination}`);
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.renameSync(from, to);
  console.log(`${source} -> ${destination}`);
}

// 内联单元测试仍作为被测模块的子模块编译，保留私有成员访问，不扩大公开 API。
// 桌面设计器收尾时仅迁移依赖文件读取职责，不重跑历史目录迁移。
if (process.argv.includes('--workflow-designer')) {
  split('src-tauri/src/runtime/manager.rs', 'src-tauri/src/runtime/bundle.rs',
    '/// 从已保存文档加载完整的可达依赖', null,
    '//! 从工作目录冻结可达依赖，校验草稿与磁盘修订。\nuse crate::document::Workspace;\nuse argusflow_workflow::{Action, WorkflowBundle, WorkflowId};\nuse std::collections::{BTreeMap, BTreeSet};\n\n');
  process.exit(0);
}

const inlineTests = [
  ['argusflow-core', 'operation.rs', 'operation/ticket.rs', 'operation.rs'],
  ['argusflow-windows', 'query.rs', 'uia/query.rs', 'uia/query.rs'],
  ['argusflow-windows', 'native.rs', 'platform/com.rs', 'platform/com.rs'],
  ['argusflow-windows', 'window_stamp.rs', 'window/stamp.rs', 'window/stamp.rs'],
  ['argusflow-windows', 'input/inject.rs', 'input/inject.rs', 'input/inject.rs'],
  ['argusflow-windows', 'input/keyboard.rs', 'input/keyboard.rs', 'input/keyboard.rs'],
  ['argusflow-windows', 'input/submission.rs', 'input/submission.rs', 'input/submission.rs'],
  ['argusflow-vision', 'components.rs', 'ocr/detection/components.rs', 'ocr/detection/components.rs'],
  ['argusflow-vision', 'detection.rs', 'ocr/detection/postprocess.rs', 'ocr/detection/postprocess.rs'],
  ['argusflow-vision', 'decode.rs', 'ocr/recognition/decode.rs', 'ocr/recognition/decode.rs'],
  ['argusflow-vision', 'image_input.rs', 'image/input.rs', 'image/input.rs'],
];
for (const [crate, oldFile, newFile, testFile] of inlineTests) {
  const destination = `tests/${crate}/unit/${testFile}`;
  if (fs.existsSync(resolve(destination))) continue;
  const source = resolve(`crates/${crate}/src/${oldFile}`);
  const text = fs.readFileSync(source, 'utf8');
  const marker = '\n#[cfg(test)]\nmod tests {\n';
  const start = text.indexOf(marker);
  if (start < 0 || !text.trimEnd().endsWith('}')) throw new Error(`测试块不符合预期：${source}`);
  const body = text.slice(start + marker.length, text.lastIndexOf('}'))
    .split('\n').map(line => line.startsWith('    ') ? line.slice(4) : line).join('\n');
  fs.mkdirSync(path.dirname(resolve(destination)), { recursive: true });
  fs.writeFileSync(resolve(destination), body.trimEnd() + '\n');
  const relative = path.posix.relative(`crates/${crate}/src/${path.posix.dirname(newFile)}`, destination);
  fs.writeFileSync(source, text.slice(0, start) + `\n\n#[cfg(test)]\n#[path = "${relative}"]\nmod tests;\n`);
}

const sources = {
  'argusflow-core': {
    'geometry.rs': 'action/geometry.rs',
    'input.rs': 'action/input.rs',
    'error.rs': 'operation/error.rs',
    'operation.rs': 'operation/ticket.rs',
  },
  'argusflow-windows': {
    'action.rs': 'uia/action.rs',
    'element.rs': 'uia/element.rs',
    'query.rs': 'uia/query.rs',
    'runtime.rs': 'uia/runtime.rs',
    'worker.rs': 'uia/worker.rs',
    'window.rs': 'window/identity.rs',
    'window_stamp.rs': 'window/stamp.rs',
    'native.rs': 'platform/com.rs',
    'ownership.rs': 'platform/ownership.rs',
  },
  'argusflow-browser': {
    'browser.rs': 'browser/client.rs',
    'endpoint.rs': 'browser/endpoint.rs',
    'connection.rs': 'cdp/connection.rs',
    'transport.rs': 'cdp/transport.rs',
    'protocol.rs': 'cdp/protocol.rs',
    'lifecycle.rs': 'cdp/lifecycle.rs',
    'cleanup.rs': 'cdp/cleanup.rs',
    'managed.rs': 'process/managed.rs',
    'process_job.rs': 'process/job.rs',
    'page.rs': 'page/handle.rs',
    'element.rs': 'page/element.rs',
    'input.rs': 'page/input.rs',
  },
  'argusflow-vision': {
    'config.rs': 'engine/config.rs',
    'engine.rs': 'engine/service.rs',
    'ownership.rs': 'engine/ownership.rs',
    'models.rs': 'model/loader.rs',
    'image_input.rs': 'image/input.rs',
    'pipeline.rs': 'ocr/pipeline.rs',
    'result.rs': 'ocr/result.rs',
    'components.rs': 'ocr/detection/components.rs',
    'detection.rs': 'ocr/detection/postprocess.rs',
    'decode.rs': 'ocr/recognition/decode.rs',
    'preprocessing.rs': 'ocr/preprocessing.rs',
  },
};
for (const [crate, mapping] of Object.entries(sources)) {
  for (const [from, to] of Object.entries(mapping)) {
    move(`crates/${crate}/src/${from}`, `crates/${crate}/src/${to}`);
  }
}

const tests = [
  ['crates/argusflow-browser/src/tests/mod.rs', 'tests/argusflow-browser/unit/mod.rs'],
  ['crates/argusflow-browser/src/tests/managed.rs', 'tests/argusflow-browser/unit/managed.rs'],
  ['crates/argusflow-browser/src/tests/ownership.rs', 'tests/argusflow-browser/unit/ownership.rs'],
  ['crates/argusflow-browser/src/tests/page.rs', 'tests/argusflow-browser/unit/page.rs'],
  ['crates/argusflow-browser/src/tests/transport.rs', 'tests/argusflow-browser/unit/transport.rs'],
  ['crates/argusflow-browser/tests/native.rs', 'tests/argusflow-browser/integration/native.rs'],
  ['crates/argusflow-browser/tests/ownership.rs', 'tests/argusflow-browser/integration/ownership.rs'],
  ['crates/argusflow-windows/src/runtime_tests.rs', 'tests/argusflow-windows/unit/uia/runtime.rs'],
  ['crates/argusflow-windows/tests/native.rs', 'tests/argusflow-windows/integration/native.rs'],
  ['crates/argusflow-windows/tests/support/mod.rs', 'tests/argusflow-windows/support/window.rs'],
  ['crates/argusflow-windows/examples/test_window.rs', 'tests/argusflow-windows/support/test_window.rs'],
  ['crates/argusflow-vision/src/engine_tests.rs', 'tests/argusflow-vision/unit/engine/service.rs'],
  ['crates/argusflow-vision/tests/native.rs', 'tests/argusflow-vision/integration/native.rs'],
  ['crates/argusflow-vision/tests/fixtures/bilingual.png', 'tests/argusflow-vision/fixtures/bilingual.png'],
];
for (const [from, to] of tests) move(from, to);

// 同一文件中的配置、查询和测试替身也按职责拆分；边界均为明确源码标记。
function split(source, destination, startMarker, endMarker, imports, rewrite = value => value) {
  if (fs.existsSync(resolve(destination))) return;
  const text = fs.readFileSync(resolve(source), 'utf8');
  const start = text.indexOf(startMarker);
  const end = endMarker === null ? text.length : text.indexOf(endMarker, start);
  if (start < 0 || end <= start) throw new Error(`拆分边界不存在：${source}`);
  fs.mkdirSync(path.dirname(resolve(destination)), { recursive: true });
  fs.writeFileSync(resolve(destination), imports + rewrite(text.slice(start, end)));
  fs.writeFileSync(resolve(source), text.slice(0, start) + text.slice(end));
}
split('crates/argusflow-browser/src/browser/client.rs', 'crates/argusflow-browser/src/browser/config.rs',
  '/// CDP 连接资源预算。', 'struct Inner {',
  '//! CDP 连接预算与显式浏览器启动选项。\nuse crate::BrowserError as Failure;\nuse argusflow_core::FailureKind;\nuse std::{path::PathBuf, time::Duration};\n\n',
  value => value.replace('    fn validate(', '    pub(super) fn validate('));
split('crates/argusflow-windows/src/uia/runtime.rs', 'crates/argusflow-windows/src/uia/config.rs',
  '/// UIA 资源和超时上限。', '/// UIA 实例的只读生命周期状态。',
  '//! UIA 遍历、租约和 Provider 资源预算。\nuse std::time::Duration;\n\n');
split('crates/argusflow-windows/src/window/identity.rs', 'crates/argusflow-windows/src/window/locator.rs',
  '/// 顶层窗口的只读快照。', null,
  '//! 顶层窗口枚举、筛选和只读快照。\nuse super::identity::WindowIdentity;\nuse crate::{WindowsError as Failure, platform::{failure, hwnd}};\nuse argusflow_core::FailureKind;\nuse windows::{core::BOOL, Win32::{Foundation::{HWND, LPARAM}, UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible}}};\n\n');
split('tests/argusflow-browser/unit/mod.rs', 'tests/argusflow-browser/support/mock.rs',
  'use crate::', null, '//! 本地 CDP WebSocket 服务端替身。\n',
  value => value
    .replace('struct Mock {', 'pub(super) struct Mock {')
    .replace('    connection: Connection,', '    pub(super) connection: Connection,')
    .replace('    socket: WebSocketStream<TcpStream>,', '    pub(super) socket: WebSocketStream<TcpStream>,')
    .replaceAll('    async fn ', '    pub(super) async fn ')
    .replace('fn options(', 'pub(super) fn options('));

// 只移除本次显式迁移留下的空目录，不递归删除任何文件。
for (const relative of [
  'crates/argusflow-browser/src/tests', 'crates/argusflow-browser/tests',
  'crates/argusflow-windows/tests/support', 'crates/argusflow-windows/tests',
  'crates/argusflow-vision/tests/fixtures', 'crates/argusflow-vision/tests',
]) {
  const directory = resolve(relative);
  if (fs.existsSync(directory) && fs.readdirSync(directory).length === 0) fs.rmdirSync(directory);
}
