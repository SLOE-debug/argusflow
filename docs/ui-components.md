# UI 基础组件

从 `src/components/ui` 公共入口导入。控件只依赖 React、Lucide、Tailwind CSS 与类名合并工具，不读取工作流或画布状态。外观使用主题语义颜色；默认高度 32px，Input／Select 的 `controlSize="compact"` 为 28px。

带图标的搜索输入统一使用 `Input` 的 `leading` 插槽，图标与输入文字共享控件边框。`Menu` 接收区分动作、分隔线和子菜单的 `MenuItem` 联合类型；`menu/` 分别管理生命周期、每层焦点和视口定位。菜单项高 28px，支持悬停展开、方向键导航、逐级返回、外部点击关闭与边缘翻转。

| 组件               | 契约与行为                                                                                      |
| ------------------ | ----------------------------------------------------------------------------------------------- |
| Input、Textarea    | div 外壳与原生文本输入；保留 `value/onChange`、输入法、选区与原生 ref，`className` 调整外壳     |
| Select             | `options/value/onValueChange`，选项包含唯一字符串 `value`、`label` 和可选 `disabled/searchText` |
| Checkbox           | `checked/onCheckedChange/label`；支持 `indeterminate`                                           |
| RadioGroup         | `options/value/onValueChange/label`；单一 Tab 入口，方向键跳过禁用项                            |
| Switch             | `checked/onCheckedChange/label`                                                                 |
| FormField          | `label/htmlFor/error`；复杂字段也可以作为具名组组织                                             |
| Button、IconButton | 共用动作和焦点样式；IconButton 必须提供 `aria-label`                                            |

选择类控件提交值而非 DOM 事件；不提供原生 `<option>` 或旧 `onChange` 的兼容接口。Input 和 Textarea 的文本内核仍接受原生 `onChange`。

```tsx
<Select
  aria-label="执行方式"
  value={mode}
  options={[
    { value: "sequential", label: "依次执行" },
    { value: "parallel", label: "并行执行" },
  ]}
  onValueChange={setMode}
/>

<Checkbox
  label="记录详细日志"
  checked={verbose}
  onCheckedChange={setVerbose}
/>
```

Select 支持方向键、Home／End、Enter／Space、Escape、Tab 和文本前缀查找。导航不立即提交；确认后提交并保留触发器焦点。浮层按视口空间翻转和限高，在原生模态 dialog 内挂载到该 dialog，避免被模态层遮挡或设为 inert。

使用 `disabled` 声明禁用状态，`aria-invalid` 声明错误状态。错误文本放入 FormField，必要时通过 `aria-describedby` 与具体控件关联。业务组件不重复绘制基础控件，也不直接依赖组件内部文件。

自动验证使用 Vitest/jsdom，覆盖交互与可访问语义，不能替代真实 WebView 的视觉、中文输入法实机和高 DPI 验收。
