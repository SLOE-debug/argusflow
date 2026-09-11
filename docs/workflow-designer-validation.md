# Workflow 设计器验证记录

日期：2026-09-11，Windows x64。本记录对应当前未提交的 Tauri 设计器、基础 UI、自绘节点树与自动应用数据存储实现；较早的引擎验收见 [Workflow 验证](workflow-validation.md)。

## 检查结果

| 检查                                                    | 结果                                                  |
| ------------------------------------------------------- | ----------------------------------------------------- |
| `pnpm test`                                             | 14 个测试文件，55 通过、0 失败                        |
| `pnpm typecheck`                                        | 通过；桌面构建也再次执行 TypeScript 检查              |
| `cargo test --workspace`                                | 164 通过、0 失败、11 ignored                          |
| 其中桌面宿主测试                                        | 8 项单元测试、8 项文件集成测试通过                    |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过                                                  |
| `cargo fmt --all -- --check`                            | 通过                                                  |
| `pnpm tauri build --no-bundle`                          | 通过，含 TypeScript 检查、Vite 生产构建及 release EXE |

## 新增与回归范围

- 基础 UI：自绘下拉键盘选择、禁用与空选项、Escape／Tab、外部点击、模态 dialog 内挂载、视口边界定位；文本组合输入事件、选区与错误语义；复选框、单选组和开关的值回调。
- 工作台导航：标题栏标签切换／关闭，校验与运行仅在工作流编辑区显示，状态栏提供面板开关与主题选择；空画布仅显示添加引导，有节点后显示起止标记；欢迎页和侧栏新建流程，侧栏流程打开和刷新、节点树搜索与折叠恢复、方向键和 Enter 添加。
- 常用节点：频次与最近顺序、默认补足、偏好读回与损坏／写入失败处理；点击、画布拖放和搜索成功后计数，输入法确认、失败、只读、撤销重做、复制粘贴不计数。
- 自动初始化：并发合并、幂等调用不丢失草稿、首次创建目录、旧路径偏好不参与初始化、加载损坏文档明确报错、失败后重试。
- 真实文件权限：仅对测试临时目录设置拒绝创建文件的 Windows ACL，验证初始化拒绝不可写目录，再恢复 ACL 验证重试成功；不会修改真实 AppData 或用户工作流。
- 画布：连续缩放进入和退出容器、正确作用域命中、连线及路由、容器删除、引用重写、复制粘贴和撤销重做；常驻平移工具不修改节点、居中显示保留缩放；二级菜单在线路落点插入节点并保留后继，排列操作单次撤销。
- 级联菜单：禁用项不执行、各层键盘焦点隔离、方向键展开与 Escape 返回、悬停切换、外部点击关闭、实际浮层尺寸避让视口边缘。
- 编辑与主题：紧凑属性面板、输入框与组合输入的键盘边界、主题扩展、系统配色切换。
- AQL：参数重新打开后恢复、草稿随撤销重做同步、旧异步响应不能清除新输入或覆盖后来修改的配置、已删除参数的草稿随应用查询一起清理。
- 保存：重叠保存串行化、保存期间的新修改、冲突时保留草稿、另存副本后解除冲突阻塞、文件原子替换、整数无损及非法路径拒绝。
- 跨工作流执行：独立根帧、内部子流程绑定正确全局、缺失和递归依赖拒绝、失败阻断后续节点、取消与总超时、借用资源只清理一次。
- 宿主日志：事件消费完后发布最终快照、完整输出无损编码、订阅先收到快照、通道失效后仍能查询最终结果、重复或其他运行的消息不会重复写入前端日志。
- 故障与退出：单运行限制、失败时展开日志、最终错误和清理错误遵守 5000 条日志上限、事件缺口保留、关闭等待超时后保留句柄并允许重试。

前端组件交互使用 Vitest/jsdom；Monaco 运行时装配测试也在测试环境内执行，没有打开或操作用户浏览器。宿主测试使用真实 WorkflowEngine 与不装配原生服务的 AutomationHost，Tauri Channel 使用受控接收器。

## Ignored 项

| 类别                   | 数量 | 本轮处理                                 |
| ---------------------- | ---- | ---------------------------------------- |
| 真实 UIA / 键鼠        | 2    | 未获本轮实际桌面操作授权，未执行         |
| 真实浏览器启动与所有权 | 2    | 遵守禁止主动浏览器验收的约定，未执行     |
| OCR CPU/CUDA           | 2    | 本轮未准备和核验原生模型运行环境，未执行 |
| GPU/DXGI               | 3    | 本轮未获硬件验收授权，未执行             |
| 专属应用进程生命周期   | 2    | 本轮未单独执行，历史记录不计入本轮       |

11 项均仍注册在工作区测试中，没有通过删除、迁移漏编译或取消 ignored 来改变数量。

## 复现

```powershell
pnpm test
pnpm typecheck
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm tauri build --no-bundle
```

本轮工作区测试日志位于忽略提交的 `target/workflow-ui-workspace-tests.log`。桌面构建产物为 `target/release/argusflow.exe`，不提交二进制。真实 WebView 视觉、高 DPI、真实输入法、完整真实浏览器流程、桌面输入、GPU 和 OCR 交互没有在本轮验证；DOM 测试不作为视觉验收。

构建保留两条非阻断提示：Monaco 按需加载的运行时分块约 3 MiB（压缩后约 779 KiB）；MSVC 链接器输出导入库创建消息。没有隐藏提示或放宽严格 Clippy 规则。

## 开发启动扫描范围修复

同日标题栏与滚动条调整后再次执行 `pnpm test`（14 文件、55 通过、0 失败）及 `pnpm build`（含 TypeScript 检查），均通过。导航测试从完整工作台验证欢迎页／侧栏新建、标题栏校验与运行的禁用状态、标签切换与关闭；滚动条样式及菜单边框高度修复已构建，未进行 WebView 视觉验收。

2026-09-11 使用 Vite `createServer` 在临时端口启动，各次使用独立的 `node_modules/.vite-startup-audit-*` 缓存，通过 `transformRequest('/src/styles.css')` 测量首次样式转换，并等待依赖扫描及当前转换请求结束。没有打开浏览器或启动桌面窗口。

| 本机单次测量 | 修改前 | 修改后 |
| --- | --- | --- |
| 首次样式转换 | 4,397 ms | 155 ms |
| 从监听启动到上述准备完成 | 4,792 ms | 223 ms |
| 采样时监听的 `target` 子目录 | 3,407 | 0 |

原依赖扫描实际包含 `target/release/build/argusflow-desktop-*/out/tauri-codegen-assets/*.html`。现在通过 `optimizeDeps.entries` 固定入口，文件监听排除 Rust 和构建目录，Tailwind 明确以 `src` 为扫描根并包含 `index.html`。依据：[Vite 依赖扫描入口](https://vite.dev/config/dep-optimization-options#optimizedeps-entries)、[Tauri Vite 配置](https://v2.tauri.app/start/frontend/vite/)、[Tailwind 源文件检测](https://tailwindcss.com/docs/detecting-classes-in-source-files)。

额外确认监听稳定后 `target` 仍为 0，`main.tsx`、`styles.css`、`Button.tsx` 和 `StudioApp.tsx` 仍在监听范围。`pnpm build` 通过（包含 WASM 构建和 TypeScript 检查），保留已有 Monaco 大分块提示。以上数据是本机前端准备阶段测量，未测量完整 `pnpm start` 到窗口显示的耗时，也不代表 Rust 首次编译时间。
