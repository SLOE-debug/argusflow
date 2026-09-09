# 项目结构

源码按职责分层；根目录 `tests/` 是全仓库唯一的测试文件位置。公共 Rust 类型继续由各 crate 的 `lib.rs` 导出，调用方不依赖内部文件路径。

```text
crates/
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
    capture/
      gpu/                  设备、精确差分、分块历史、完成检测和读回
      output/               duplication、候选区、版本发布
      requests/             区域读取和版本比较任务
      worker.rs             每适配器采集及有限恢复编排
      lease.rs              来源撤销与 GPU 资源释放
    uia/                    配置、查询、Pattern、租约、MTA worker
    window/                 窗口定位、身份、HWND 标记
    input/                  输入服务、事件构造、注入、部分失败清理
    platform/               COM 生命周期、Win32 错误、实例所有权
  argusflow-browser/src/
    browser/                配置、启动与连接入口、端点解析
    cdp/                    连接、协议收发、请求关联、会话状态、清理
    page/                   页面、DOM 元素、输入
    process/                自建进程、Job、配置目录回收
  argusflow-vision/src/
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
  argusflow-core/unit/
  argusflow-capture-contracts/unit/
  argusflow-capture/
    unit/                   精确区域算法
    integration/            版本、稳定、历史、缺口与取消
    support/                确定性像素来源与故障注入
  argusflow-windows/
    unit/{uia,window,platform,input,capture}/
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
  refactor-layout.mjs       本次目录迁移的可重复显式映射
```

单元测试通过 `#[cfg(test)]` 和 `#[path]` 作为原模块的子模块编译，保留私有成员访问权限；`src/` 中只有挂载声明。集成测试由各 crate 的 `[[test]]` 指向根目录，仍使用原测试目标名，因此 `cargo test --workspace`、`--test native` 和 `--test ownership` 的用法不变。验收窗口仍可通过 `--example test_window` 运行。

运行 `node scripts/refactor-layout.mjs` 可重复执行初版目录迁移；目标已存在时跳过已完成步骤，源/目标冲突时停止，不覆盖已有目标。迁移只改变组织与引用，不改变协议或算法。开发与验收命令见 [验证记录](validation.md)。
