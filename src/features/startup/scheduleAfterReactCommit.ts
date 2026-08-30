/**
 * 在 React 提交启动页后的下一个宏任务启动后台能力。
 *
 * 原始 HTML Loading 在模块加载前已经可见；这里仅让当前 effect 返回，并允许
 * 开发模式 StrictMode 取消第一次探测任务，不额外等待动画帧。
 */
export function scheduleAfterReactCommit(task: () => void): () => void {
  /** 尚未执行的初始化任务句柄，用于组件卸载和 StrictMode 探测清理。 */
  const pendingTask = window.setTimeout(task, 0);
  return () => window.clearTimeout(pendingTask);
}
