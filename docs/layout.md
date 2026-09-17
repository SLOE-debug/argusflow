# 项目结构

源码按职责分层；根目录 `tests/` 是全仓库唯一的测试文件位置。公共 Rust 类型继续由各 crate 的 `lib.rs` 导出，调用方不依赖内部文件路径。

```text
crates/
  argusflow-input-contracts/src/
    event.rs                真实输入事实、来源、窗口与坐标契约
  argusflow-recorder/src/
    model/                  记录、观察、状态、像素及OCR身份
    normalize/              操作归一化、时间共现索引和写者编排
    storage/                校验追加日志、恢复、相对附件与回看缓存
    protocol.rs             父子私有管道命令与通知
    policy.rs               可解释的前后截图规则
  argusflow-workflow/src/
    model/                  流程文档、控制节点与表达式树
    value/                  强类型值与空间计量
  argusflow-runtime/src/
    compilation/            图所有权、词法绑定、调用依赖与端口检查
    expression/             纯表达式类型检查与有界求值
    execution/              显式帧栈、数据事务、调用和异常展开
    contract/               扩展任务与执行错误
    resource/               资源依赖与失败清理保留
  argusflow-workflow-automation/src/
    query/                  AQL 准备、绑定和快照适配
    browser/                浏览器任务
    desktop/                Windows 应用与窗口任务
    resources/              类型化资源和宿主共享服务
  argusflow-aql/src/
    model/                  角色、属性、表达式和冻结参数
    syntax/                 保留标记的词法与英文解析
    checking/               比较类型与表达式输出检查
    evaluation/             能力、预算树、匹配及排序
    language/               格式化、符号说明、悬浮与英文语言服务
    diagnostic.rs           诊断和 UTF-8/UTF-16 位置
  argusflow-aql-wasm/src/
    localization/           中文词汇与双向位置映射
    service.rs              中文草稿分析
    hover.rs                中英文说明与参数类型的位置回写
    completion.rs           中文候选、英文筛选词与插入片段
    bridge.rs               窄 WASM 导出
  argusflow-automation/src/
    source/                 UIA、DOM、OCR 来源装配
    locator/                唯一定位与动作编排
  argusflow-core/src/
    action/                 坐标、鼠标键盘参数
    operation/              截止时间、取消、错误上下文
  argusflow-capture-contracts/src/
    source/                 时钟、版本、来源描述与平台边界
    geometry.rs             本地矩形、屏幕物理像素和旋转
    image.rs                带资源租约的只读共享图像
    resource.rs             原子字节预算
    sampling.rs             视觉消费者契约
    frames.rs               独立全屏帧流、缓存快照和缩放预算契约
  argusflow-image/src/
    view.rs                 借用像素视图、格式与区域校验
    difference.rs           通道差分、四连通区域及可配置过滤
  argusflow-capture/src/
    sampling.rs             完整帧历史的区域稳定采样与内容令牌
    pixels.rs               原图裁剪与公共精确比较
    validity.rs             来源生命周期有效性
  argusflow-windows/src/
    application/            自有进程 Job、参数编码和窗口等待
    capture/
      frames/               办公全屏帧采集、缓存及独立生命周期
      video/                原生GPU硬件视频、QPC索引、本地分片落盘及按需解码
      gpu/                  设备、纹理资源与完整帧缩放旋转
    uia/                    配置、查询、Pattern、租约、MTA worker
      aql/                  属性、关系、几何和聚焦验证
    window/                 窗口定位、身份、HWND 标记
    input/                  输入服务、事件构造、注入、部分失败清理
    listening/              真实低级Hook、消息线程、暂停与有界队列
    platform/               COM 生命周期、Win32 错误、实例所有权
  argusflow-browser/src/
    browser/                配置、启动与连接入口、端点解析
    cdp/                    连接、协议收发、请求关联、会话状态、清理
    page/                   页面、DOM 元素、输入
      aql/                  DOM/AX、frame/Shadow 会话、几何及动作
    process/                自建进程、Job、配置目录回收
  argusflow-vision/src/
    aql/                    OCR 文字与置信度查询
    sampling/               稳定区域 OCR、在途合并、有界缓存
    engine/                 配置、工作线程、引擎所有权
    image/                  图片解码和像素输入
    model/                  官方模型校验与 ORT 会话加载
    ocr/
      detection/            连通区域与 DB 后处理
      recognition/          CTC 解码
      preprocessing.rs      张量预处理与透视裁剪
      pipeline.rs           单图推理编排
      cache.rs              上次成功图像与文字块复用判定
      incremental.rs        公共差分、完整检测与增量识别编排
      result.rs             文本块与阅读顺序
tests/
  argusflow-recorder/{unit,integration,support}/ 日志故障、归一化、策略、迁移读取和基准
  argusflow-desktop/{unit,integration,support}/
  argusflow-runtime/{unit,integration,support,fixtures}/
  argusflow-workflow-automation/{unit,integration,support,fixtures}/
  argusflow-aql/integration/
  argusflow-aql-wasm/integration/
  argusflow-automation/{unit,integration,support}/
  frontend/{aql,workflow,recorder,support}/ 草稿、组件、画布、WASM、录制关联及固定 DOM 脚本测试
  argusflow-core/unit/
  argusflow-capture-contracts/unit/
  argusflow-image/unit/      精确差分、格式与录制过滤策略
  argusflow-capture/
    integration/            稳定、内容复用、来源撤销和取消
    support/                确定性完整帧来源
  argusflow-windows/
    unit/{application,uia,window,platform,input,capture}/
    integration/            真实 UIA、输入、DXGI 验收
    support/                测试专属窗口、采样 OCR 示例
  argusflow-browser/
    unit/                   CDP 失败路径与生命周期测试
    integration/            真实浏览器操作和资源回收
    support/                本地 WebSocket 替身
  argusflow-vision/
    unit/{engine,image,ocr,sampling}/
    support/                区域来源替身
    integration/            官方模型 CPU/CUDA 验收
    fixtures/               固定测试图片
scripts/
  build-aql-wasm.ps1         WASM 编译与前端绑定生成
  refactor-layout.mjs       本次目录迁移的可重复显式映射
```

前端源码在 `src/`：`components/ui/` 为通用控件，`components/editor/` 为 Monaco 生命周期，`components/workflow/` 按工作区、画布、配置、节点库、数据和执行展示拆分。`flow/` 保留业务无关的几何、选择、键盘与路由；`features/workflow/` 持有模型、节点、值表达式、传输和 Studio 编排；`features/themes/` 提供主题契约、注册表及切换。

UI 输入、选择、单选／复选、开关和字段容器按控件分文件，`components/ui/select/` 分离选择交互与浮层定位。`components/workflow/palette/` 分离流程列表、节点树和可见行推导；`workspace/WorkflowTabs.tsx` 提供注入标题栏的标签导航。`features/workflow/studio/workspace.ts` 编排自动初始化，`nodes/usage.ts` 持有常用统计。对应测试位于根目录 `tests/frontend/{ui,workflow,support}/`。

`features/aql/` 保存 WASM 契约和中文草稿状态；`features/aql/generated/` 由构建脚本生成并忽略提交。入口 `main.tsx` 只装配主题和页面。

画布使用原生 Canvas 2D：`components/workflow/canvas/rendering/` 保存主场景、卡片、圆弧连线、移动障碍预览和资源生命周期，交互与浮层分别由画布目录的 Hook 管理。`wireGesture.ts` 推导四边吸附与改接预览，`hitTest.ts` 按相同曲线几何命中。`features/workflow/model/canvas-scene.ts` 派生嵌套作用域与可缓存路径；`connections.ts`、`node-creation.ts`、`node-deletion.ts` 分别管理连线约束、拆线插入与删除续接，`validation.ts` 管理保存结构错误。`flow/geometry/` 分离锚点、障碍索引、正交搜索和圆弧几何，不依赖工作流业务。测试集中于 `tests/frontend/workflow/canvas/` 与 `edge-*.test.ts`，绘图和布局替身位于 `tests/frontend/support/`。相机及编辑契约见 [画布实现](workflow-canvas.md)。

`src-tauri/src/document/` 负责文件、结构和无损传输编译；`runtime/assembly.rs` 装配平台服务，`runtime/bundle.rs` 读取冻结依赖，`runtime/manager.rs` 持有运行生命周期，`runtime/journal.rs` 管理有序日志与最终结果。`commands.rs` 只适配命令参数；`lib.rs` 和 `main.rs` 装配应用。依赖加载职责拆分可通过 `node scripts/refactor-layout.mjs --workflow-designer` 重复执行。

`src-tauri/src/recorder/` 分离子进程写者、父进程托管、心跳、管道和读包命令；`evidence/` 负责 UIA 生命周期与结构、页面属性，`context.rs` 负责时间线单项操作与关联结构证据读取。`src/features/recorder/` 持有协议、API、状态、有限缓存、事件帧选择与 `frame-map.ts` 的点击/OCR坐标契约；`src/components/recorder/` 提供录制菜单、状态摘要、事件画面及按需文字坐标详情。

单元测试通过 `#[cfg(test)]` 和 `#[path]` 作为原模块的子模块编译，保留私有成员访问权限；`src/` 中只有挂载声明。集成测试由各 crate 的 `[[test]]` 指向根目录，仍使用原测试目标名，因此 `cargo test --workspace`、`--test native` 和 `--test ownership` 的用法不变。验收窗口仍可通过 `--example test_window` 运行。

运行 `node scripts/refactor-layout.mjs` 可重复执行初版目录迁移；目标已存在时跳过已完成步骤，源/目标冲突时停止，不覆盖已有目标。迁移只改变组织与引用，不改变协议或算法。开发与验收命令见 [验证记录](validation.md)。

`src-tauri/src/recorder/video/`负责视频子进程、片段索引和回看命令；`capture/video/decode.rs`核对原生解码PTS与显示区域；`argusflow-recorder/storage/review_cache.rs`只管理可重建图片缓存。旧 `pipeline.rs` 批量 PNG 链及其测试已删除；`video/analysis/regions.rs` 调用公共 `argusflow-image` 变化区域算法。
