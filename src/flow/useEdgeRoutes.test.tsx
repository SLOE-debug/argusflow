import { act, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FlowEdge, FlowNode } from './types';
import { useEdgeRoutes } from './useEdgeRoutes';

/** 可主动返回路由结果的最小 Worker 测试替身。 */
class RoutingWorkerMock {
  public static readonly instances: RoutingWorkerMock[] = [];

  /** Worker 收到的全部文档请求。 */
  public readonly messages: unknown[] = [];
  /** 当前注册的 message 监听器。 */
  private readonly listeners = new Set<(event: MessageEvent) => void>();

  public constructor() {
    RoutingWorkerMock.instances.push(this);
  }

  public postMessage(message: unknown): void {
    this.messages.push(message);
  }

  public addEventListener(
    type: string,
    listener: (event: MessageEvent) => void,
  ): void {
    if (type === 'message') this.listeners.add(listener);
  }

  public removeEventListener(
    type: string,
    listener: (event: MessageEvent) => void,
  ): void {
    if (type === 'message') this.listeners.delete(listener);
  }

  public terminate(): void {}

  /** 模拟 Worker 完成一次精确路由。 */
  public respond(revision: number): void {
    const event = { data: { revision, routes: [] } } as MessageEvent;
    for (const listener of this.listeners) listener(event);
  }
}

type RouteHarnessProps = Readonly<{
  nodes: ReadonlyArray<FlowNode>;
  edges: ReadonlyArray<FlowEdge>;
}>;

/** 挂载路由 Hook 并暴露当前预览数量。 */
function RouteHarness({ nodes, edges }: RouteHarnessProps) {
  const routes = useEdgeRoutes(nodes, edges);
  return <output>{routes.length}</output>;
}

describe('useEdgeRoutes worker backpressure', () => {
  /** 等待执行的动画帧回调。 */
  let frames: Map<number, FrameRequestCallback>;
  /** 下一个动画帧 ID。 */
  let nextFrameId: number;

  beforeEach(() => {
    RoutingWorkerMock.instances.length = 0;
    frames = new Map();
    nextFrameId = 0;
    vi.stubGlobal('Worker', RoutingWorkerMock);
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      const frameId = ++nextFrameId;
      frames.set(frameId, callback);
      return frameId;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn((frameId: number) => {
      frames.delete(frameId);
    }));
  });

  afterEach(() => vi.unstubAllGlobals());

  /** 运行当前排队的全部动画帧。 */
  const flushFrames = () => {
    const callbacks = [...frames.values()];
    frames.clear();
    for (const callback of callbacks) callback(16);
  };

  it('keeps one in-flight request and only the latest pending snapshot', () => {
    const edges: FlowEdge[] = [];
    const createNodes = (x: number): FlowNode[] => [{
      id: 'a',
      kind: 'test',
      position: { x, y: 0 },
      size: { width: 80, height: 50 },
      data: null,
    }];
    const view = render(
      <RouteHarness
        nodes={createNodes(0)}
        edges={edges}
      />,
    );
    const worker = RoutingWorkerMock.instances[0];
    act(flushFrames);
    expect(worker.messages).toHaveLength(1);

    view.rerender(<RouteHarness nodes={createNodes(10)} edges={edges} />);
    act(flushFrames);
    view.rerender(<RouteHarness nodes={createNodes(20)} edges={edges} />);
    act(flushFrames);
    expect(worker.messages).toHaveLength(1);

    act(() => worker.respond(1));
    expect(worker.messages).toHaveLength(2);
    expect(worker.messages[1]).toMatchObject({ revision: 3 });
  });
});
