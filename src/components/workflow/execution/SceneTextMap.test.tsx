import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { VisualQueryTrace } from '../../../features/workflow';
import { SceneTextMap } from './SceneTextMap';

describe('SceneTextMap', () => {
  it('renders the contact conversation and sent message from the captured projection', () => {
    render(<SceneTextMap trace={messageScene()} />);

    expect(screen.getByText('文件传输助手')).toBeInTheDocument();
    expect(screen.getByText('ArgusFlow 测试消息')).toBeInTheDocument();
  });
});

function messageScene(): VisualQueryTrace {
  return {
    schema_version: 2,
    run_id: 'run-1',
    node_id: 'check_send_result',
    node_sequence: 85,
    query: 'exists(text(name = $message))',
    outcome: 'unique',
    candidate_nodes: [{ window_handle: '100', scene_id: 7, node_id: 'message' }],
    selected_node: { window_handle: '100', scene_id: 7, node_id: 'message' },
    metrics: { elapsed_us: 120, exact_index_hits: 1, scanned_nodes: 2, spatial_candidates: 1 },
    projection: {
      schema_version: 2,
      windows: [{
        window_handle: '100',
        scene_id: 7,
        frame_id: 15,
        z_order: 0,
        foreground: true,
        screen_bounds: { x: 0, y: 0, width: 900, height: 700 },
        frame_bounds: { x: 0, y: 0, width: 900, height: 700 },
      }],
      nodes: [
        sceneNode('contact', '文件传输助手', 420, 30),
        sceneNode('message', 'ArgusFlow 测试消息', 640, 540),
      ],
    },
  };
}

function sceneNode(nodeId: string, text: string, x: number, y: number) {
  return {
    node_id: nodeId,
    scene_id: 7,
    frame_id: 15,
    window_handle: '100',
    text,
    frame_bbox: { x, y, width: 180, height: 30 },
    screen_bbox: { x, y, width: 180, height: 30 },
    polygon: [],
    confidence: 0.96,
    source: 'ocr_small',
  };
}
