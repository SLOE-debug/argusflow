import { Component, type ReactNode } from "react";
import { Button } from "../../ui";

/** 场景数据异常只隔离画布，用户仍可通过外部操作栏撤销或重新载入。 */
export class CanvasErrorBoundary extends Component<
  { readonly children: ReactNode },
  { readonly failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: Error) {
    console.error("画布场景无法构建", error);
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <section className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 bg-canvas">
        <p role="alert" className="text-sm text-danger">
          画布无法显示。可撤销最近的修改后重试。
        </p>
        <Button onClick={() => this.setState({ failed: false })}>
          重试画布
        </Button>
      </section>
    );
  }
}
