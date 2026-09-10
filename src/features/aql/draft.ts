import type { DocumentAnalysis, DraftState } from './contracts';

/** 可独立测试的草稿控制器，组合输入期间不进行格式化和导出。 */
export class DraftController {
  private state: DraftState;
  private composing = false;
  private active = true;
  private readonly listeners = new Set<() => void>();

  constructor(source: string, private readonly analyze: (source: string) => Promise<DocumentAnalysis>) {
    this.state = { status: 'pending', source, revision: 0 };
  }

  /** 供 useSyncExternalStore 读取的不可变快照。 */
  getSnapshot = (): DraftState => this.state;
  /** 订阅只读状态变更。 */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  /** 更新文本时立即撤销旧的英文导出。 */
  edit(source: string): void {
    this.state = { status: 'pending', source, revision: this.state.revision + 1 };
    this.publish();
    if (!this.composing) void this.refresh();
  }
  /** 输入法候选状态不会被分析结果回写打断。 */
  composition(active: boolean): void {
    this.composing = active;
    this.state = { status: 'pending', source: this.state.source, revision: this.state.revision + 1 };
    this.publish();
    if (!active) void this.refresh();
  }
  /** 开始分析；取消通过版本判定实现，不保留过期结果。 */
  async refresh(): Promise<void> {
    const { source, revision } = this.state;
    if (!this.active || this.composing) return;
    try {
      const analysis = await this.analyze(source);
      if (!this.active || this.composing || this.state.revision !== revision) return;
      this.state = { status: 'ready', source, revision, analysis };
    } catch {
      if (!this.active || this.state.revision !== revision) return;
      this.state = { status: 'failed', source, revision, message: '暂时无法检查语法。请刷新页面重试，当前草稿仍保留在编辑器中。' };
    }
    this.publish();
  }
  /** 撤销组件卸载后的结果通知。 */
  dispose(): void { this.active = false; this.listeners.clear(); }
  private publish(): void { for (const listener of this.listeners) listener(); }
}
