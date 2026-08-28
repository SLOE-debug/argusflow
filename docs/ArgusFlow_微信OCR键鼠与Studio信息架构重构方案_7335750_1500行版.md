# ArgusFlow 微信 OCR + 键鼠执行链路与 Studio 信息架构重构方案

> 仓库：`SLOE-debug/argusflow`
> 基线：`main @ 7335750b62563f7b49cf605db626685ba5f177b0`
> 基线提交：`feat: 落地微信视觉感知与 PaddleOCR 执行链路`
> 日期：2026-08-28
> 目标：在已经落地 Vision / PaddleOCR 的前提下，把微信默认工作流从“固定等待 + 键盘盲操作”升级为“视觉观察 + OCR 确认 + 键鼠执行 + 动作后验证”；同时重新整理 Studio 左侧节点库、预设入口、流程控制分组和左右面板折叠交互。
> 范围：设计与实施方案，不在本文直接提交代码。
> 核心约束：不新增第二套 Runtime，不把 OCR/坐标/FindElement 暴露成用户必须拼接的低层节点，不破坏现有 `NodeEnvelope / NodeTypeRegistry / PreparedPlan / AppSession / ResourceTable / ValueExpr / VisionRuntime` 架构。
## 0. 结论先行

本轮建议同时收敛两条主线：**微信执行闭环** 与 **Studio 信息架构**。

微信侧的核心变化不是“增加一个 OCR 节点”，而是让现有语义 UI 节点能够使用视觉事实完成定位，并在执行动作时由 SendInput 做最终键鼠注入：

```text
Workflow Semantic Node
    Click / GetText / Extract / PressKey / TypeText
              |
              v
       PreparedPlan / Router
              |
      +-------+-------------------------+
      |                                 |
      v                                 v
Vision observe / materialize       SendInput actuation
VisualScene -> 0/1/N -> bbox       keyboard / mouse
      |                                 |
      +------------ verify -------------+
```

微信默认流程不再使用“250ms / 600ms / 500ms”这种猜测式 Delay 来等待 UI，而改成：

```text
打开微信
-> Ctrl+F
-> 等搜索界面视觉目标出现
-> 输入群名
-> OCR 唯一确认搜索结果
-> 视觉定位并鼠标点击该结果
-> OCR 精确确认聊天 Header
-> 输入消息
-> Enter 发送
-> OCR 验证新消息已出现
```

Studio 左侧应明确区分“节点”和“可复用资产”：

```text
底部 Tab
├─ 节点     <- 只放 Primitive / Semantic Node
├─ 预设     <- Node Preset + Flow Component
├─ 资源     <- 后续工作区资源管理
└─ 设置
```

因此：

- 从“节点库”中移除顶部的“快捷操作”和“可复用流程”两个分组。
- 删除当前没有实际价值且只是占位的“流程大纲” Tab。
- 将“流程分支”和“等待”合并进“流程控制”。
- 将 Delay 改名为“固定暂停”，明确它不是 UI readiness wait。
- 左右面板都增加点击收起按钮。
- 顶部标题栏增加左侧面板 / 底部 Dock / 右侧面板三个全局显隐按钮。
- 面板关闭时保留最后宽度；重新打开恢复原宽度。
- 不新增 OCR 专用画布节点；视觉是定位/观察后端，不是业务工作流层级。
- 不让用户手工拼“OCR 读坐标 -> 坐标点击”；视觉 bbox 是执行期瞬时事实。
- 不把“发送微信群消息”硬编码成 Primitive；它适合作为 Flow Component，出现在新的“预设”页。
## 1. 最新 main 的真实状态与本方案出发点

### 1.1 Vision 已经不再是空壳

最新提交已经把视觉执行链路真正接进仓库，主要包括：

- `argusflow-vision` 的视觉事实模型。
- 稳定帧门控。
- 低分辨率差分与 Dirty ROI。
- `VisualScene` cache。
- OCR 文本投影。
- 视觉 exact / contains 查询。
- Windows Graphics Capture 窗口捕获。
- 窗口拓扑与 DPI 处理。
- SendInput 鼠标输入。
- PaddleOCR 3.7.0 Python worker。
- Named Pipe 协议、健康检查、预热。
- `VisionBackend / VisionRuntime / Evidence` 与 Tauri runtime 装配。

这意味着现在不应该再规划“先做一套 OCR 基础设施”，而应该直接解决**视觉事实如何成为 UI 动作的定位事实**。

### 1.2 当前微信模板仍然是开环键盘脚本

当前默认微信模板仍然大致是：

```text
Start
-> 打开微信
-> Ctrl+F
-> Delay 250ms
-> Ctrl+A
-> TypeText(group_name)
-> Delay 600ms
-> Enter
-> Delay 500ms
-> TypeText(message)
-> Enter
-> End
```

其中三个 Delay 都是在吸收微信异步渲染时间。

这个流程的问题不是“延时数值不够准”，而是它没有任何事实证明：

- Ctrl+F 后搜索界面真的出现了。
- 输入群名后目标群真的成为唯一搜索结果。
- Enter 以后打开的是预期群聊。
- 发送前 Header 真的是目标群。
- Enter 以后新消息真的进入聊天历史。

因此当前流程更像键盘宏，而不是可审计自动化。

### 1.3 当前 VisionBackend 只承担观察

最新实现里 `VisionBackend` 只处理：

```text
GetText
GetValue
Extract
```

`Click(Visual)` 仍不会由 VisionBackend 直接执行。

这个限制本身是对的：Vision 应负责观察和 materialize，User32/SendInput 才负责物理输入。

真正缺失的是两者之间的一条稳定桥：

```text
Visual target
-> resolve current stable scene
-> exact 0/1/N
-> transient bbox / safe point
-> SendInput click
```

### 1.4 当前 SendInput 已经具备关键执行能力

SendInput 目前已支持：

- `PressKey(Focused)`。
- `TypeText(Focused)`。
- `Click(Coordinate)`。

所以本轮不需要重新实现键盘和鼠标基础设施。

需要做的是让 `Click(Visual)` 在执行期被 materialize 为可信的屏幕点，然后复用现有 `inject_click`。

### 1.5 当前节点库确实混合了三种概念

节点库的数据结构当前同时包含：

- Core/Semantic Node。
- Node Preset。
- Flow Component。

并把 Preset / Component 作为节点分组放在最上面。

这和仓库已有“Primitive / Preset / Component / Backend”分层哲学冲突：Preset 与 Component 是**创建资产/复用资产**，不是 Runtime Primitive 分类。

因此截图里红框区域让人感觉“不像节点”，这个判断是正确的。
## 2. 本轮必须守住的架构边界

1. **语义节点优先**：画布节点表达“点击、读取、输入、条件、资源获取”等稳定语义，不表达 Win32 API 步骤。
2. **Preset 不进入 Runtime**：Node Preset 只是单个 Primitive 的默认参数与展示包装，拖入后就是普通节点。
3. **Component 才负责业务复用**：“发送微信群消息”“网页链接采集”属于可版本化 Flow Component。
4. **Vision 是事实层**：OCR 输出 VisualScene / bbox / confidence / region，不直接成为业务流程控制流。
5. **Actuation 单一归属**：键盘、鼠标、滚轮等物理注入继续归属 argusflow-windows::input。
6. **等待属于需要它的节点**：为了让当前目标出现而等待，使用 target_wait；Delay 只保留固定暂停。
7. **0/1/N 必须确定**：视觉目标 0 个可等待，1 个可执行，N 个立即 Ambiguous；禁止最高分偷偷获胜。
8. **非幂等动作必须验证**：发消息、提交、删除等动作失败时不能盲目重试。
9. **动态数据使用 ValueExpr**：群名、消息等运行输入不能退化为写死字符串。
10. **面板状态与面板尺寸分离**：隐藏不等于 width=0 的永久状态；重新打开应恢复用户原宽度。

## 3. Studio 左侧信息架构：从“节点分类”与“资产入口”混合改为两层导航

### 3.1 当前问题

当前 `nodePaletteCatalog.ts` 的顺序大致是：

```text
快捷操作        <- Preset
可复用流程      <- Component
触发流程
流程分支
等待
应用和浏览器
操作界面
运行程序
处理数据
输出结果
```

这会产生三个认知问题。

第一，用户会把“点击”这种 Preset 当成一种和“条件判断”“打开应用”同层级的 Runtime node type。

第二，“可复用流程”本质上是资产库，而不是节点类别；它应该有版本、来源、搜索、更新、详情等行为，未来肯定会比一个 palette group 更重。

第三，节点页面顶部被复用资产占据，导致真正的 Primitive 分类被压到下面，截图中的红框视觉上也因此像“特殊区块”。

### 3.2 推荐底部 Tab

将左侧底部导航收敛为：

```text
[节点] [预设] [资源] [设置]
```

四个 Tab 的职责：

```text
节点
  只展示可直接进入画布的语义节点 / Primitive。

预设
  Node Preset：点击、输入文字、读取文字、读取控件值、读取网页链接……
  Flow Component：网页链接采集、发送微信群消息、登录 ERP……

资源
  工作流凭据、文件、应用别名、连接、环境等后续资产管理入口。

设置
  Studio / Workspace / 编辑器偏好。
```

### 3.3 删除“流程大纲”

当前 `outline` 只是占位模块，而且在节点型 DAG 编辑器里缺少明显增量价值。

流程大纲的问题：

- 画布本身已经是结构表达。
- DAG 没有天然树层级，所谓“按层级查看”需要人为排序。
- 大流程真正需要的是搜索、Minimap、组件下钻、运行定位，而不是再维护一份树。
- 当前没有实现，因此保留只会增加一个无反馈入口。

本轮建议直接删除 `outline` 模块。

未来如果真实需求出现，不要恢复成“层级大纲”，更适合做：

```text
全局搜索 / Cmd+K
节点结果定位
错误列表
运行时间线
组件调用关系
```

### 3.4 “子流程”改名为“预设”而不是单独保留

当前底部已有 `subflows`，但这仍然只覆盖 Component。

用户反馈希望把“快捷操作 + 可复用流程”都移到底部，因此建议把 `subflows` 直接替换为 `presets`。

预设页内部明确二级分区：

```text
常用操作
  点击
  输入文字
  读取文字
  读取控件值
  读取网页链接

流程组件
  官方组件
  工作区组件
  我的组件（未来）
```

这既满足产品心智，又不破坏内部 NodePreset / FlowComponent 的类型区分。
## 4. 节点页重新分组：不要把流程控制拆得过细

### 4.1 推荐分组

节点页建议只保留五个主组：

```text
流程控制
资源
界面与浏览器
系统
数据与输出
```

如果后期节点数量明显增加，再从五组扩成六到七组，而不是现在为了每 1~2 个节点单独建一组。

### 4.2 流程控制

建议包含：

```text
开始
条件判断
固定暂停
结束
```

未来再加入：

```text
循环
并行
重试边界
异常处理
```

“流程分支”和“等待”没有必要各占一个 group。

### 4.3 Delay 改名“固定暂停”

当前“等待”很容易被理解为“等待目标出现”。

但仓库自己的 readiness wait 设计已经明确：

> 为了让当前节点满足执行前置条件而等待，应属于当前节点 target_wait。

因此 palette 文案建议：

```text
标题：固定暂停
描述：无条件暂停指定时间后继续
```

Inspector 里也应写：

```text
这不是“等待目标出现”。
如果你在等待按钮、文字或搜索结果，请在对应界面节点中开启“等待目标”。
```

### 4.4 资源

```text
打开应用
打开浏览器
```

后续如果有数据库连接、SSH 会话等，再扩充；当前“打开网页 Navigate”更像浏览器动作，不建议和 Acquire Browser 放在资源组里。

### 4.5 界面与浏览器

```text
操作界面
打开网页
```

其中“操作界面”仍然是 `argus.ui` 语义容器；Palette 里的“点击”等易用快捷入口去预设页。

### 4.6 系统

```text
执行命令
```

未来：文件、HTTP、进程等。

### 4.7 数据与输出

```text
设置变量
整理文本
记录日志
查看结果
```

未来数据节点变多时再拆“数据处理”和“输出结果”。
## 5. 左右面板与顶部全局折叠：状态模型

### 5.1 当前布局问题

`App.tsx` 目前只有：

```text
libraryWidth
inspectorWidth
dockOpen
```

没有：

```text
libraryOpen
inspectorOpen
```

所以左右面板只能 resize，不能真正 collapse。

左侧 NodePalette header 的 `PanelLeft` 按钮当前实际作用是“恢复默认宽度”，这在视觉上又很像“收起面板”，容易产生误导。

### 5.2 新状态

建议引入：

```ts
type StudioLayoutState = {
  libraryOpen: boolean;
  inspectorOpen: boolean;
  dockOpen: boolean;
  libraryWidth: number;
  inspectorWidth: number;
  dockHeight: number;
};
```

本轮可以继续放在 `App.tsx`，等布局偏好稳定后再抽 `useStudioLayout`。

### 5.3 Grid 规则

```ts
gridTemplateColumns:
  `${libraryOpen ? libraryWidth : 0}px minmax(0,1fr) ${inspectorOpen ? inspectorWidth : 0}px`
```

注意：

- `libraryWidth` 在关闭时不改成 0。
- `inspectorWidth` 在关闭时不改成 0。
- 重新打开直接恢复最后一次宽度。
- ResizeHandle 仅在面板打开时渲染。
- 不用宽度动画，避免 FlowCanvas 每帧重新测量和重新路由。

### 5.4 左侧本地收起按钮

NodePalette header 右侧改成真正的：

```text
[收起左侧面板]
```

使用类似：

```text
PanelLeftClose
```

原“恢复节点库宽度”按钮移除。

恢复默认宽度保留在：

- ResizeHandle 双击。
- 或设置页“恢复默认布局”。

### 5.5 右侧本地收起按钮

NodeInspector header 增加：

```text
属性        [当前选择] [收起]
```

使用类似：

```text
PanelRightClose
```

右侧收起以后，Canvas 自动占用释放出来的空间。

### 5.6 顶部全局显隐按钮

`EditorToolbarControls` 当前只有撤销 / 重做，且注释已经声明它可以承载 panel commands，因此最适合增加：

```text
撤销  重做 | 左侧面板  底部面板  右侧面板
```

三个按钮使用 `aria-pressed`：

```text
PanelLeft
PanelBottom
PanelRight
```

含义统一：

- pressed=true：面板可见。
- pressed=false：面板隐藏。
- 点击可双向 toggle。

### 5.7 为什么顶部和面板内都要有按钮

面板内按钮解决“我正在看这个面板，想收起来”。

顶部按钮解决“面板已经没了，我要找回来”。

两者缺一不可，否则隐藏后只能靠快捷键或设置恢复。

### 5.8 快捷键建议

不强制本轮实现，但建议预留：

```text
Ctrl+B        左侧面板
Ctrl+J        底部 Dock
Ctrl+Shift+B  右侧属性
```

如果与已有命令冲突，以全局命令表为准。
## 6. 微信 V2：从“键盘宏”升级为“Observe → Decide → Act → Verify”

### 6.1 目标流程

建议默认开发模板改成：

```text
Start
-> Acquire Application(微信)
-> PressKey(Ctrl+F)
-> GetText(Visual: 搜索界面关键文字, target_wait=bounded)
-> PressKey(Ctrl+A)
-> TypeText(group_name)
-> GetText(Visual: group_name, exact, region=SearchResults, target_wait=bounded)
-> Click(Visual: group_name, exact, region=SearchResults)
-> GetText(Visual: group_name, exact, region=Header, target_wait=bounded, medium if needed)
-> TypeText(message)
-> PressKey(Enter)
-> VerifyNewOutgoingMessage(message)
-> End
```

P0 如果还没有通用 `VerifyNewOutgoingMessage` 契约，则先用一个保守的视觉读取/验证步骤，后续再升成正式 verification primitive/strategy。

### 6.2 为什么打开群聊改用视觉点击而不是 Enter

Enter 依赖：

- 当前焦点仍在搜索结果。
- 排序没有变化。
- 首条结果恰好是目标。
- 微信没有把焦点切到其它控件。

而视觉点击可以把“目标群”作为明确事实：

```text
OCR exact unique match
-> bbox
-> safe point
-> mouse click
```

这比“按 Enter 打开第一条”更符合可审计自动化。

### 6.3 为什么仍保留 Ctrl+F / Ctrl+A / TypeText

键盘快捷键是稳定、低成本、跨视觉状态的动作。

Vision 不应该为了“看起来全 OCR”而替代一切键盘语义。

推荐优先级：

```text
稳定快捷键 > 视觉鼠标点击 > 绝对坐标
```

### 6.4 搜索阶段必须确认两次

第一次确认：搜索结果里目标群唯一出现。

第二次确认：进入聊天后 Header 精确等于目标群。

这两次事实不同：

```text
Search result match = 选择意图正确
Header match        = 实际会话正确
```

只有第二次确认成功，才允许输入和发送消息。

### 6.5 发送前与发送后

发送前：

- 已确认 Header。
- 可选强模式：OCR 编辑器区域，确认 draft 文本。

发送后：

- 等 ChatHistoryBottom 变化并稳定。
- OCR bottom region。
- 确认是本次发送后新出现的消息，而不是历史中的同文本。

如果验证不确定：

```text
返回 Uncertain / 明确失败
不要自动再次 Enter
```

因为发送消息是非幂等动作。
## 7. 微信工作流中应彻底删除的三类做法

- 删除 `wechat_wait_search_1` 这类 readiness Delay。
- 删除“搜索结果出来后直接 Enter 第一项”的隐式目标选择。
- 删除“发送后不验证就成功结束”的开环判定。
- 不要新增“微信 OCR 搜索群”这种 WeChat 专属 Runtime node type。
- 不要新增“OCR 获取坐标”用户节点。
- 不要新增“按坐标点击 OCR 结果”用户节点组合。
- 不要把 OCR backend 选择暴露成默认必填参数。
- 不要在 worker 内部决定点击哪一个候选。
- 不要让 fuzzy 最高分自动变成动作目标。
- 不要用全桌面 OCR 代替 AppSession WindowSet。

## 8. 关键契约缺口一：VisualQuery 必须支持动态 ValueExpr

### 8.1 当前 VisualQuery 的限制

当前视觉查询本质上是：

```rust
VisualQuery {
  text: String,
  exact: bool,
}
```

前端也是：

```ts
{ type: 'visual', query: { text: string, exact: boolean } }
```

这对静态“确定 / 搜索 / 发送”可以工作，但无法直接表达：

```text
查找运行输入 group_name
查找运行输入 message
```

如果不改，会被迫在默认模板里把群名写死，或者新增业务专用节点，这都不合适。

### 8.2 推荐：持久化查询使用 ValueExpr，运行时仍冻结为 String

不要把 `argusflow-vision` 改成会解释 ValueExpr。

正确分层：

```text
Workflow payload
VisualQueryExpr { text: ValueExpr, exact, region? }
        |
Runtime prepare
resolve ValueExpr
        |
Resolved VisualQuery { text: String, exact, region? }
        |
VisionBackend
```

推荐新契约：

```rust
pub struct VisualQueryExpr {
    pub text: ValueExpr,
    pub exact: bool,
    pub region: Option<NormalizedRect>,
}
```

或保持 TargetLocator 层的命名：

```rust
TargetLocatorSpec::Visual { query: VisualQueryExpr }
ResolvedTargetLocator::Visual { query: VisualQuery }
```

如果仓库不希望引入第二套 Locator 类型，也可以只让 `UiNodeCompiler` 在 payload decode 后把 ValueExpr 解析掉，再构造现有 `AutomationTarget`。

### 8.3 迁移策略

旧 v2 UI payload：

```json
{ "text": "确定", "exact": true }
```

新 v3 UI payload：

```json
{
  "text": { "type": "literal", "value": "确定" },
  "exact": true
}
```

编译器迁移：

```text
old string -> ValueExpr::Literal(String)
```

不需要升整个 workflow schema，只升 `argus.ui` payload 版本。

### 8.4 Inspector 交互

视觉目标文字字段不再是普通 `<Input>`，改为复用 `ValueExprFields`：

```text
目标文字来源
  常量 / 上游输出 / 工作流输入 / 变量 / 表达式
```

这样默认微信模板可直接选：

```text
group_name
message
```
## 9. 关键契约缺口二：视觉查询需要可选 Region，避免同文案歧义

### 9.1 为什么仅 text + exact 不够

微信同一群名可能同时出现在：

- 左侧会话列表。
- 搜索结果。
- 顶部 Header。
- 历史消息正文。

如果只做全窗口 exact 查询，即使 OCR 全对，也很容易得到 N>1 的 AmbiguousTarget。

### 9.2 P0 推荐 NormalizedRect

无需现在设计完整视觉 DSL。

增加一个可选归一化区域：

```rust
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

范围全部约束到 0..1，相对目标 VisualSurface。

用途：

```text
SearchOverlay/SearchResults
Header
Editor
ChatHistoryBottom
```

### 9.3 Region 是查询范围，不是固定点击坐标

Region 只缩小 OCR/query 候选集合。

最终点击点仍来自当次 VisualScene 中目标节点 bbox。

所以：

```text
region ≠ coordinate click
```

窗口 resize/DPI 改变时，region 仍可随 surface 尺寸映射。

### 9.4 Inspector

默认折叠在“更多视觉设置”：

```text
识别范围
  整个应用窗口
  自定义区域

x 0.00
y 0.00
w 1.00
h 1.00
```

P1 再做“从截图拖框选择区域”。
## 10. 关键执行桥：让 Click(Visual) 走 Vision materialize + SendInput

### 10.1 不建议让 VisionBackend 直接注入鼠标

如果让 `argusflow-vision` 直接依赖 User32 并执行 click，会破坏：

```text
Vision = observe/materialize
Windows input = actuation
```

也会让输入前 HWND/PID / foreground 校验出现两套实现。

### 10.2 P0 推荐引入窄接口 VisualTargetResolver

建议在 `argusflow-agent` 或一个不反向依赖 platform crate 的层定义：

```rust
#[async_trait]
pub trait VisualTargetResolver: Send + Sync {
    async fn resolve(
        &self,
        window: WindowIdentity,
        query: &VisualQuery,
        policy: VisualResolvePolicy,
    ) -> Result<ResolvedVisualTarget, AutomationError>;
}
```

结果：

```rust
pub struct ResolvedVisualTarget {
    pub window: WindowIdentity,
    pub scene_id: SceneId,
    pub frame_id: FrameId,
    pub bbox: PhysicalRect,
    pub confidence: f32,
    pub safe_point: ScreenPoint,
}
```

注意：这些都只是 execute-time transient fact，不写回 workflow。

### 10.3 Resolver 实现归属

`VisionRuntime` 或 `VisionTargetResolver` 实现该 trait：

```text
current stable scene
-> optional region filter
-> exact/contains 0/1/N
-> VisualNode bbox
-> client/screen coordinate mapping
-> safe point
```

### 10.4 SendInputBackend 增加 Visual Click 支持

当前支持：

```text
Click(Coordinate)
```

扩为：

```text
Click(Coordinate)
Click(Visual)
```

Visual click prepare 时冻结：

- query。
- AppSession window context。
- resolver handle。
- input policy。

execute 时：

```text
1. 复验 HWND -> PID。
2. 激活/确认前台窗口。
3. resolver 单次 materialize。
4. 若 target region 已 dirty，强制取最新 stable scene。
5. 0 -> TargetNotFound。
6. 1 -> safe point。
7. N -> AmbiguousTarget。
8. inject_click。
9. 返回 ActionOutcome。
```

### 10.5 PreparedPlan 仍负责 target_wait

Resolver 自己不要偷偷 5 秒循环。

一次 materialize 没找到，返回 `TargetNotFound`。

外层 PreparedPlan 根据 UI node 的 `target_wait` 决定是否再尝试。

这样：

```text
UIA / CDP / Vision
```

全部共享同一 deadline 语义。

### 10.6 safe point 策略

优先级：

```text
row hit rect safe center
> expanded text hit rect safe center
> text bbox center
```

避免长期依赖：

```text
文字中心 + 固定 27 像素
```

这种魔法偏移无法适应 DPI、字体、布局变化。
## 11. 视觉读取、视觉点击、键盘输入的 Backend 责任表

| 语义动作 | 定位 | 观察/解析 | 物理执行 | 备注 |
|---|---|---|---|---|
| GetText | Visual | VisionRuntime | 无 | exact 0/1/N |
| Extract | Visual | VisionRuntime | 无 | Many 可返回集合 |
| Click | Visual | VisionTargetResolver | SendInput | 本轮关键新增 |
| Click | Coordinate | 无 | SendInput | 已有能力 |
| PressKey | Focused | AppSession/foreground | SendInput | 已有能力 |
| TypeText | Focused | AppSession/foreground | SendInput | 已有能力 |
| SetValue | Query | UIA/CDP | UIA/CDP | 语义快路径 |
| Click | Query | UIA/CDP | UIA/CDP | UIA/CDP 可用时优先 |

这张表的核心是：**定位后端和执行后端不一定是一回事。**

SendInput 是 actuation backend，不应该被简单理解成“selector fallback 的最后一档”。
## 12. Verification：微信消息发送必须成为显式闭环

### 12.1 P0 最小目标

先把发送后的成功条件从“Enter 调用成功”改成“画面出现预期新消息”。

### 12.2 推荐抽象

长期建议：

```rust
enum VerificationPolicy {
    None,
    BestEffort,
    Required,
}

enum VerificationOutcome {
    Confirmed,
    Rejected(String),
    Uncertain(String),
}
```

视觉条件：

```text
TextExists
TextAbsent
NewTextExistsSince
HeaderEquals
```

### 12.3 为什么 TextExists(message) 不够

同样文本可能已经存在于历史消息。

所以生产级发送确认至少要关联：

```text
before_send_scene_id
+ ChatHistoryBottom region
+ NewTextExistsSince(message)
```

P1 再叠加：

```text
outgoing/right alignment hint
message block aggregation
```

### 12.4 不确定时的规则

```text
Confirmed -> 成功
Rejected  -> 失败
Uncertain -> 停止并返回需要人工判断
```

绝对禁止：

```text
Uncertain -> 再按一次 Enter
```
## 13. Readiness wait：微信流程中不再出现猜测式 Delay

### 13.1 搜索界面

Ctrl+F 以后，不 `sleep(250ms)`。

改为：

```text
GetText(Visual search marker)
target_wait = bounded(3000~5000ms)
```

目标 120ms 出现，就尽快继续。

### 13.2 搜索结果

TypeText(group_name) 以后，不 `sleep(600ms)`。

改为：

```text
GetText(Visual group_name exact, region=SearchResults)
target_wait = bounded
```

### 13.3 群聊加载

点击搜索结果以后，不 `sleep(500ms)`。

改为：

```text
GetText(Visual group_name exact, region=Header)
target_wait = bounded
```

### 13.4 什么时候仍应使用“固定暂停”

例如：

- API 限流要求明确暂停 2 秒。
- 演示录屏故意停留 1 秒。
- 业务协议明确要求退避。
- 测试夹具模拟人为节奏。

这些才是 Delay。
## 14. 默认微信模板 V2：建议节点清单

| ID | 展示名 | 语义 | 说明 |
|---|---|---|---|
| `start_1` | 开始 | `start` | 进入流程。 |
| `wechat_application_1` | 打开微信 | `application` | AttachOrStart，产出 AppSession。 |
| `wechat_open_search_1` | 打开搜索 | `ui.press_key` | Ctrl+F，Focused + SendInput。 |
| `wechat_verify_search_1` | 确认搜索界面 | `ui.get_text.visual` | 等待搜索界面标志出现。 |
| `wechat_select_search_text_1` | 选中搜索文字 | `ui.press_key` | Ctrl+A。 |
| `wechat_type_group_name_1` | 输入群名称 | `ui.type_text` | ValueExpr <- workflow_input.group_name。 |
| `wechat_find_group_1` | 确认搜索结果 | `ui.get_text.visual` | VisualQueryExpr.text <- group_name；SearchResults region。 |
| `wechat_click_group_1` | 打开群聊 | `ui.click.visual` | 同一 group_name exact unique；Vision -> bbox -> SendInput。 |
| `wechat_verify_header_1` | 确认群聊 | `ui.get_text.visual` | Header region exact == group_name。 |
| `wechat_type_message_1` | 输入消息 | `ui.type_text` | ValueExpr <- workflow_input.message。 |
| `wechat_send_message_1` | 发送消息 | `ui.press_key` | Enter；非幂等动作。 |
| `wechat_verify_message_1` | 确认已发送 | `verification/visual` | 新消息只允许确认一次，不做自动重发。 |
| `end_1` | 结束 | `end` | 只有验证通过后正常结束。 |

## 15. 默认模板里的运行输入与测试数据

保留：

```text
group_name
message
```

建议把默认运行输入从具体群名改为更中性的开发占位：

```json
{
  "group_name": "ArgusFlow 测试群",
  "message": "ArgusFlow 自动化测试消息"
}
```

或者让 `group_name` 默认为空，在运行前强制用户填写。

原因：

- 默认仓库不应携带看起来像真实个人群聊的示例值。
- 自动化 demo 应尽量减少误发风险。
- Production mode 未来可要求明确勾选“允许发送消息”。
## 16. 新“预设”页的产品结构

### 16.1 顶部结构

```text
预设
[搜索预设________________]

常用操作
  点击
  输入文字
  读取文字
  读取控件值
  读取网页链接

流程组件
  网页链接采集
  发送微信群消息
```

### 16.2 Node Preset 行为

点击/拖入“点击”：

```text
创建普通 argus.ui 节点
operation.type = click
```

拖入以后，节点不再保留“我是 preset”的 Runtime 身份。

### 16.3 Flow Component 行为

拖入“发送微信群消息”：

```text
创建 argus.component instance
pin component_id + exact version
```

双击可以下钻查看内部原子流程。

### 16.4 Badge

在预设页卡片上建议显示：

```text
操作预设
流程组件
```

而不是继续在节点页混合显示。

### 16.5 来源

P1 可增加：

```text
官方
工作区
我的
```

以及版本、更新提示。
## 17. “发送微信群消息”作为 Flow Component，而不是 Primitive

建议新增官方组件：

```text
name: 发送微信群消息
version: 1.0.0
inputs:
  group_name: text
  message: text
outputs:
  confirmed: bool/json
  evidence?: json
```

内部节点就是第 14 节的闭环流程。

它符合 Component 的理由：

- 包含多个节点。
- 有清晰业务语义。
- 输入/输出稳定。
- 可以独立版本化。
- 可以下钻审计。
- 未来可以升级内部 OCR/验证策略，而不是制造更多 Primitive。

主画布可因此非常简洁：

```text
Start
-> [发送微信群消息]
-> End
```

但默认开发模板建议暂时保留“展开版本”，便于当前阶段调试 Vision/SendInput。
## 18. 前端代码改动建议

| 文件 | 改动 |
|---|---|
| `src/App.tsx` | 新增 `libraryOpen / inspectorOpen`；grid 依据 open 状态使用 0 列；只在打开时渲染 resize handle；把 panel toggle 传给标题栏命令。 |
| `src/components/workflow/palette/PaletteNavigation.tsx` | `PaletteModule` 改为 nodes/presets/resources/settings；删除 outline；subflows 改 presets。 |
| `src/components/workflow/palette/NodePalette.tsx` | 节点页不再拼入 preset/component；接收 onCollapse；非 nodes 模块改成真实 PresetCatalogView 或资源/设置占位。 |
| `src/components/workflow/palette/nodePaletteCatalog.ts` | 移除 preset/component PaletteGroup；合并 control/advanced；重新分组。 |
| `src/components/workflow/palette/PresetCatalogView.tsx` | 新增预设页；渲染 NODE_PRESET_CATALOG 与 Flow Component catalog。 |
| `src/components/workflow/inspector/NodeInspector.tsx` | header 增加 onCollapse 按钮。 |
| `src/components/workflow/workspace/toolbar/EditorToolbarControls.tsx` | 增加左/下/右 panel toggles，aria-pressed。 |
| `src/components/workflow/index.ts` | 导出新增 PresetCatalogView / panel command props。 |
| `src/features/workflow/model/contracts.ts` | Visual query text 改成 ValueExpr 形式或新增持久化 VisualQueryExpr；增加 optional region。 |
| `src/features/workflow/nodes/workflowAction.ts` | 支持 VisualQueryExpr 的默认值、切换、ValueExpr 迁移。 |
| `src/components/workflow/inspector/node-fields/ActionNodeFields.tsx` | 视觉目标文字改 ValueExprFields；增加可选 region 高级设置。 |
| `src/features/workflow/model/defaultWorkflowTemplate.ts` | 删除三个 Delay；新增视觉确认、视觉点击、发送后验证。 |
| `src/features/workflow/components/componentCatalog.ts` | 新增官方“发送微信群消息”组件定义。 |
| `src/features/workflow/nodes/nodePresetCatalog.ts` | 内容可保留，但入口只从预设页进入。 |

## 19. Rust / Runtime 代码改动建议

| 文件/模块 | 改动 |
|---|---|
| `crates/argusflow-core/src/automation.rs` | 增加持久化动态视觉查询所需结构；运行时 resolved query 继续是 String；可选 NormalizedRect。 |
| `crates/argusflow-runtime/src/builtin_nodes/ui.rs` | 在 prepare 阶段解析 visual query ValueExpr，并构造 resolved AutomationAction。 |
| `crates/argusflow-agent/src/...` | 定义窄 `VisualTargetResolver` / `ResolvedVisualTarget` 契约，避免 windows 反向依赖 vision。 |
| `crates/argusflow-vision/src/query.rs` | 增加 region 过滤；继续严格 0/1/N。 |
| `crates/argusflow-vision/src/runtime.rs` | 提供 resolver 所需 current stable scene / refresh region / materialization。 |
| `crates/argusflow-vision/src/...resolver.rs` | 建议新增，封装 query -> VisualNode -> bbox/safe_point。 |
| `crates/argusflow-windows/src/input/backend.rs` | 增加 Click(Visual) candidate；注入 resolver；execute 时 materialize 后复用 inject_click。 |
| `crates/argusflow-windows/src/input/mouse.rs` | 原则上不改语义，只复用现有安全点击与坐标注入。 |
| `src-tauri/src/runtime/...` | 装配同一个 VisionRuntime / resolver 给 VisionBackend 与 SendInputBackend。 |
| `crates/argusflow-core/src/error...` | 如尚无合适类型，补 Visual materialize / verification 的明确错误分类。 |

## 20. 不建议新增 BackendKind::VisualSendInput

看起来最直接的实现是新增：

```text
BackendKind::VisualSendInput
```

但不建议。

原因：

- Visual 描述的是定位/观察来源。
- SendInput 描述的是执行方式。
- 把二者拼进枚举会产生组合爆炸：VisualClick、VisualScroll、VisualDrag……
- 以后 UIA 找目标 + SendInput 点击也可能是合法组合。
- 以后 GUI grounding 找目标 + SendInput 点击同样需要复用。

因此应把“target materialization”和“actuation”解耦，而不是制造新的复合 backend 名字。
## 21. Visual Click 的 Prepare / Execute 详细时序

1. UiNodeCompiler 解析工作流 ValueExpr，冻结最终 `VisualQuery`。
2. ActionDispatcher 建立 ExecutionContext，冻结 AppSession 对应的 HWND/PID。
3. Router 判断该 Click 需要视觉 materialization + SendInput actuation。
4. PreparedCandidate 记录 explain：scope -> visual candidate source -> selection -> action。
5. PreparedPlan 只创建一次，target_wait 只重复执行同一 frozen plan。
6. 每次 execute_once 前复验 HWND 仍属于原 PID。
7. 确认/激活目标窗口，不允许点击到其它前台应用。
8. 调用 resolver 获取当前 stable scene。
9. 若 cache fresh 且目标 region 未 dirty，可走 VisualCache。
10. 否则 Tiny OCR 当前 query region。
11. 低置信度/关键动作需要时升级 Medium。
12. 按 region 过滤候选。
13. exact/contains 计算候选集合。
14. 0 个 -> `TargetNotFound`。
15. 1 个 -> 得到 VisualNode。
16. N 个 -> `AmbiguousTarget`，立即失败，不等待。
17. 从 VisualNode bbox/row 计算安全点击区域。
18. 将 client physical 坐标转换成 virtual screen physical 坐标。
19. 点击前再次确认窗口身份与前台状态。
20. 调用现有 `inject_click`。
21. 返回后由下一个验证节点确认状态变化。
22. 如果是非幂等动作，其后验证失败不能自动重复该动作。

## 22. Region 初始建议：只作为默认模板预设，不写成 Runtime 常量

微信布局随版本、窗口尺寸、侧边栏宽度变化而变化，因此下面只作为模板初始值，最终应由真实截图校准：

```text
SearchOverlay / SearchResults:
  x ≈ 0.00 ~ 0.45
  y ≈ 0.05 ~ 0.75

Header:
  x ≈ 0.25 ~ 1.00
  y ≈ 0.00 ~ 0.15

Editor:
  x ≈ 0.25 ~ 1.00
  y ≈ 0.70 ~ 1.00

ChatHistoryBottom:
  x ≈ 0.25 ~ 1.00
  y ≈ 0.55 ~ 0.86
```

不要把这些数字写进 `argusflow-vision`。

应该存在于：

- 默认 Flow Component 的 payload。
- 用户可编辑的视觉 query region。
- 或后续视觉区域选择器。
## 23. OCR 模型策略

- VisualCache 优先复用当前 scene，避免每个节点都重新 OCR。
- OcrTiny 负责高频 search/result/readiness。
- OcrMedium 用于低置信度升级和关键验证。
- Header 确认可直接允许 medium 优先，因为它决定是否允许发送。
- 发送后验证可 medium，优先正确性而非最低延迟。
- 模型 SKU 不应进入工作流业务语义。
- Inspector 默认只显示“自动”，backend 强制选择放高级设置。
- worker 不 Ready 时应是 RuntimeAvailability::Unavailable，不要靠 cost 假装可执行。

## 24. 失败语义与用户可见错误

| 场景 | 错误类别 | 用户文案重点 |
|---|---|---|
| 搜索界面未出现 | `TargetWaitTimeout` | 在限定时间内未识别到搜索界面。 |
| 群名 OCR 为 0 | `TargetWaitTimeout` | 未找到目标群。 |
| 群名 OCR 为 N | `AmbiguousTarget` | 发现多个同名候选，请缩小识别区域或增加条件。 |
| 点击前窗口身份变化 | `WindowIdentityChanged/BackendFailed` | 目标窗口已变化，未执行点击。 |
| Header 不匹配 | `Verification Rejected` | 已打开的聊天不是目标群，停止发送。 |
| Header OCR 不确定 | `Verification Uncertain` | 无法确认当前聊天，停止发送。 |
| 发送后未发现新消息 | `Verification Uncertain` | 无法确认消息是否发送成功，不自动重发。 |
| OCR worker 不可用 | `BackendUnavailable` | 视觉服务未就绪。 |
| 稳定帧超时 | `FrameUnstable/BackendFailed` | 界面持续变化，无法安全定位。 |

## 25. Evidence：让 OCR 键鼠流程可审计

- planner_explain.json：记录为何选择 Vision/SendInput。
- execution_context.json：记录冻结的 AppSession/HWND/PID。
- window_topology.json：记录目标窗口和 popup。
- visual_scene.json：记录节点、bbox、confidence、region。
- ocr_regions.json：记录实际 OCR ROI。
- compact_text.txt / spatial_text.txt：便于开发者快速阅读。
- ocr_overlay.png：失败时可选保存，画出 bbox 与命中候选。
- verification.json：发送前/发送后验证事实。
- 成功动作默认不持久化完整截图。
- Evidence 失败不得覆盖原始 AutomationError。

## 26. Panel collapse 的具体组件 API

建议 `NodePalette`：

```ts
type NodePaletteProps = {
  ...
  onCollapse: () => void;
};
```

建议 `NodeInspector`：

```ts
type NodeInspectorProps = {
  ...
  onCollapse: () => void;
};
```

建议 `EditorToolbarControls`：

```ts
type EditorToolbarControlsProps = {
  store: WorkflowFlowStore;
  libraryOpen: boolean;
  inspectorOpen: boolean;
  dockOpen: boolean;
  onToggleLibrary: () => void;
  onToggleInspector: () => void;
  onToggleDock: () => void;
};
```

`App.tsx` 统一掌握布局真相，避免三个子组件各自维护 open 状态导致漂移。
## 27. Layout 状态持久化

P0 可以只用 React state。

但很建议同一 PR 顺手做轻量 localStorage：

```text
key = argusflow.studio.layout.v1
```

保存：

```json
{
  "libraryOpen": true,
  "libraryWidth": 248,
  "inspectorOpen": true,
  "inspectorWidth": 312,
  "dockOpen": true,
  "dockHeight": 260
}
```

读取时：

- 对 width 再 clamp。
- schema 不认识就回退默认值。
- 不保存当前 selected node 等业务状态。

设置页未来提供“恢复默认布局”。
## 28. Node Palette 的组件拆分

当前 `NodePalette` 同时负责：

- active module。
- 搜索。
- node group。
- preset/component 拼接。
- placeholder。
- bottom navigation。

重构后推荐：

```text
NodePaletteShell
├─ PaletteHeader
├─ PaletteNavigation
└─ module body
   ├─ NodeCatalogView
   ├─ PresetCatalogView
   ├─ ResourceCatalogView
   └─ WorkspaceSettingsView
```

P0 不需要大规模搬目录，只要先把 `NodeCatalogView / PresetCatalogView` 拆出来，避免继续向单文件叠条件分支。
## 29. 节点搜索与预设搜索

“节点”与“预设”各自有搜索，但语义不同：

节点搜索：

```text
按节点名称 / 作用匹配
```

预设搜索：

```text
按预设名 / 组件名 / 描述 / 标签匹配
```

不要把两个目录拼成一个搜索结果，否则又回到混合层级。

未来全局 Cmd+K 可以跨：

```text
命令
节点
预设
组件
工作流
```

但那是另一个产品入口。
## 30. P0 / P1 / P2 范围

### P0：本轮必须完成

- 新左侧信息架构。
- 删除 outline Tab。
- Preset/Component 移入“预设” Tab。
- 合并流程控制分组。
- Delay 改名固定暂停。
- 左右面板本地收起。
- 顶部三面板显隐按钮。
- 微信模板删除 readiness Delay。
- 动态 visual query text。
- Visual query optional region。
- Click(Visual) -> resolver -> SendInput。
- 搜索结果唯一确认。
- Header 必须确认后才能发送。
- 基础发送后视觉确认。

### P1：闭环增强

- `NewTextExistsSince` 正式 verification。
- outgoing alignment / message block 聚合。
- 视觉区域截图框选 UI。
- 滚轮/智能分页接入用户节点。
- Preset/Component 版本更新 UX。
- 布局持久化与恢复默认布局。
- 全局命令搜索。

### P2：高级视觉自动化

- 关系型视觉查询。
- role_hint / spatial relation。
- GUI grounding。
- 纯图标目标。
- 视觉录制器 / selector recorder。
- 组件市场和发布治理。
## 31. 建议拆成 5 个 PR

1. **PR A — Studio IA**
   - 只改左侧信息架构：删除 outline、增加 presets、重分组、固定暂停文案；不碰 runtime。
2. **PR B — Panel Layout**
   - 左右 panel collapse + top toggles + resize handle 可见性；不碰工作流 schema。
3. **PR C — Dynamic Visual Target**
   - VisualQuery ValueExpr + optional region + inspector + v2→v3 payload migration。
4. **PR D — Visual Click Bridge**
   - VisualTargetResolver + SendInput Click(Visual) + runtime 装配 + 0/1/N tests。
5. **PR E — WeChat Closed Loop**
   - 默认模板改造 + 官方 Flow Component + Header/发送后验证 + E2E。

## 32. 为什么要这样拆 PR

- A/B 是纯 Studio 体验，可以先合并且容易回滚。
- C 是契约演进，独立测试序列化与迁移。
- D 是执行边界，集中审查安全性和 crate dependency。
- E 才把所有能力组合成微信真实流程，问题定位更清晰。
- 避免一次提交同时改 palette、schema、Vision、Windows input、默认模板，导致回归难定位。

## 33. 最终验收标准

- [ ] 节点页不再出现“快捷操作”和“可复用流程”两个 group。
- [ ] 底部导航收敛为“节点 / 预设 / 资源 / 设置”，删除“流程大纲”。
- [ ] “流程分支”和“等待”合并为“流程控制”。
- [ ] Delay 对用户显示为“固定暂停”，并明确不用于 UI readiness。
- [ ] 左侧节点面板可以在面板 header 中点击收起。
- [ ] 右侧属性面板可以在面板 header 中点击收起。
- [ ] 顶部工具区可以独立恢复/隐藏左侧、底部、右侧三个 panel。
- [ ] panel 隐藏后保留用户最后宽度，恢复时不回默认值。
- [ ] 隐藏 panel 时对应 resize handle 不再占用 pointer hit area。
- [ ] Visual query 文本可以引用 workflow input / variable / expression。
- [ ] Visual query 支持可选 normalized region，并拒绝非法范围。
- [ ] Visual query 继续严格遵守 0/1/N，不做最高分隐式选择。
- [ ] `Click(Visual)` 能通过 Vision materialize 得到 bbox/safe point，再复用 SendInput 注入鼠标。
- [ ] `Click(Visual)` 在 0 个目标时不点击，在 N 个目标时不点击。
- [ ] 点击前再次复验 HWND/PID/foreground，dirty target 不复用过期 bbox。
- [ ] 微信默认模板删除三个 readiness Delay。
- [ ] 微信搜索结果必须 OCR 唯一确认，打开群聊改为视觉点击。
- [ ] 发送消息前必须通过 Header exact verification。
- [ ] 发送后必须有视觉确认；Uncertain 时不得自动再次 Enter。
- [ ] “发送微信群消息”作为 exact-version Flow Component 出现在“预设”页，而不是新增 Primitive。
- [ ] 旧 visual string payload 可迁移为 literal ValueExpr。
- [ ] 旧 `collect_links` 与真正的 Delay 流程继续兼容。
- [ ] 至少覆盖 success / not-found / ambiguous / header-mismatch / post-send-uncertain 五类 Windows E2E。

## 34. 主要参考文件

- `docs/ArgusFlow_节点原子性与预制子流程设计方案.md`
- `docs/ArgusFlow_节点内建等待与UI就绪同步方案.md`
- `docs/ArgusFlow_微信视觉感知_PaddleOCR_v3.7_实施方案.md`
- `docs/ArgusFlow_UI与目录结构重构方案.md`
- `src/features/workflow/model/defaultWorkflowTemplate.ts`
- `src/components/workflow/palette/nodePaletteCatalog.ts`
- `src/components/workflow/palette/NodePalette.tsx`
- `src/components/workflow/palette/PaletteNavigation.tsx`
- `src/components/workflow/inspector/NodeInspector.tsx`
- `src/components/workflow/workspace/toolbar/EditorToolbarControls.tsx`
- `src/App.tsx`
- `src/features/workflow/nodes/nodePresetCatalog.ts`
- `src/features/workflow/components/componentCatalog.ts`
- `src/features/workflow/model/contracts.ts`
- `src/features/workflow/nodes/workflowAction.ts`
- `src/components/workflow/inspector/node-fields/ActionNodeFields.tsx`
- `crates/argusflow-core/src/automation.rs`
- `crates/argusflow-vision/src/runtime.rs`
- `crates/argusflow-vision/src/query.rs`
- `crates/argusflow-vision/src/backend.rs`
- `crates/argusflow-windows/src/input/backend.rs`
- `TODO.md`

## 35. 最终一句话

**节点页只放稳定语义 Primitive；预设页承载 NodePreset 与 FlowComponent；微信执行链以 Vision 负责观察/定位、SendInput 负责键鼠动作、PreparedPlan 负责等待、Verification 负责非幂等动作闭环。**
