# 项目结构

源码按职责分层；根目录 `tests/` 是全仓库唯一的测试文件位置。公共 Rust 类型继续由各 crate 的 `lib.rs` 导出，调用方不依赖内部文件路径。

```text
crates/
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
  argusflow-capture/src/
    service/                共享服务、来源状态、顺序校验、变化订阅
    observation/            锚点、过程摘要、稳定等待和净变化
    regions.rs              区域并集、裁剪、外扩、忽略区
    sampling.rs             稳定区域及内容令牌
  argusflow-windows/src/
    application/            自有进程 Job、参数编码和窗口等待
    capture/
      gpu/                  设备、精确差分、分块历史、完成检测和读回
      output/               duplication、候选区、版本发布
      requests/             区域读取和版本比较任务
      worker.rs             每适配器采集及有限恢复编排
      lease.rs              来源撤销与 GPU 资源释放
    uia/                    配置、查询、Pattern、租约、MTA worker
      aql/                  属性、关系、几何和聚焦验证
    window/                 窗口定位、身份、HWND 标记
    input/                  输入服务、事件构造、注入、部分失败清理
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
      result.rs             文本块与阅读顺序
tests/
  argusflow-runtime/{unit,integration,support,fixtures}/
  argusflow-workflow-automation/{unit,integration,support,fixtures}/
  argusflow-aql/integration/
  argusflow-aql-wasm/integration/
  argusflow-automation/{unit,integration,support}/
  frontend/{aql,support}/    草稿、组件、WASM 及固定 DOM 脚本测试
  argusflow-core/unit/
  argusflow-capture-contracts/unit/
  argusflow-capture/
    unit/                   精确区域算法
    integration/            版本、稳定、历史、缺口与取消
    support/                确定性像素来源与故障注入
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

前端源码在 `src/`：`components/ui/` 为通用控件，`components/editor/` 为工作区及 Monaco 生命周期，`features/aql/` 为 WASM 契约、草稿状态和文件操作。`features/aql/generated/` 由构建脚本生成并忽略提交；入口 `main.tsx` 只装配页面。

单元测试通过 `#[cfg(test)]` 和 `#[path]` 作为原模块的子模块编译，保留私有成员访问权限；`src/` 中只有挂载声明。集成测试由各 crate 的 `[[test]]` 指向根目录，仍使用原测试目标名，因此 `cargo test --workspace`、`--test native` 和 `--test ownership` 的用法不变。验收窗口仍可通过 `--example test_window` 运行。

运行 `node scripts/refactor-layout.mjs` 可重复执行初版目录迁移；目标已存在时跳过已完成步骤，源/目标冲突时停止，不覆盖已有目标。迁移只改变组织与引用，不改变协议或算法。开发与验收命令见 [验证记录](validation.md)。
