import type { StoreApi } from "zustand/vanilla";
import type { DesktopApi } from "../api/desktop";
import type { StudioState } from "./state";

/** 自动工作区只负责初始化状态与并发合并，不持有文档编辑行为。 */
export class WorkspaceSession {
  /** 同一次初始化被多个调用者共享；失败后释放以允许重试。 */
  private pending: Promise<void> | undefined;
  constructor(
    private readonly store: StoreApi<StudioState>,
    private readonly api: DesktopApi,
    private readonly open: (id: string) => Promise<void>,
  ) {}

  /** 启动或重试固定的应用数据工作区；已经就绪时不会重置会话。 */
  initialize(): Promise<void> {
    if (this.store.getState().initialization.status === "ready")
      return Promise.resolve();
    this.pending ??= this.load().finally(() => {
      this.pending = undefined;
    });
    return this.pending;
  }

  private async load(): Promise<void> {
    const previous = this.store.getState().initialization;
    /** 重试成功只清理本次初始化的错误，不覆盖其他服务的新提示。 */
    const previousError = previous.status === "failed" ? previous.error : null;
    this.store.setState({ initialization: { status: "loading" } });
    try {
      const result = await this.api.initializeWorkspace();
      this.store.setState({
        workspace: result.path,
        documents: result.documents,
      });
      if (!this.store.getState().active && result.documents[0])
        await this.open(result.documents[0].id);
      this.store.setState({
        initialization: { status: "ready" },
        ...(previousError && this.store.getState().message === previousError
          ? { message: null }
          : {}),
      });
    } catch (error) {
      this.store.setState({
        initialization: { status: "failed", error: String(error) },
      });
      throw error;
    }
  }
}
