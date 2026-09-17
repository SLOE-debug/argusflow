import { Component, type ReactNode } from "react";

/** 局部显示错误不能拖垮工作流工作区，也不撑开标题栏。 */
export class RecorderBoundary extends Component<
  { readonly children: ReactNode },
  { readonly failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <span
        role="alert"
        className="text-xs text-danger"
        title="录制面板显示失败，已写入的数据仍保留。重新打开应用可读取录制。"
      >
        录制不可用
      </span>
    ) : (
      this.props.children
    );
  }
}
