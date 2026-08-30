import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { BROWSER_STARTUP_SNAPSHOT } from './features/startup';

vi.mock('./components/shell/WindowTitleBar', () => ({
  WindowTitleBar: () => <header>title-bar</header>,
}));

vi.mock('./components/ui', () => ({
  PanelResizeHandle: () => null,
}));

vi.mock('./components/workflow', () => ({
  ComponentDrillDown: () => null,
  EditorPrimaryActions: () => null,
  EditorToolbarControls: () => null,
  NodeInspector: () => null,
  NodePalette: () => null,
  WorkflowCanvas: () => null,
  WorkflowOverview: () => <section>home-view</section>,
  WorkflowWorkspace: () => <section>editor-view</section>,
  WorkspaceStatusBar: () => null,
  resolveWorkflowStatus: () => 'idle',
  useWorkspaceEditor: () => ({}),
}));

vi.mock('./features/workflow', () => ({
  useWorkflowStudio: () => ({
    errorMessage: null,
    events: [],
    flowStore: {},
    nodes: [],
    report: null,
    running: false,
    workflowName: '测试流程',
  }),
}));

import App from './App';

describe('App initial view', () => {
  it('enters the home page after runtime initialization completes', () => {
    render(
      <App
        startupStatus={BROWSER_STARTUP_SNAPSHOT}
        executionEnabled
      />,
    );

    expect(screen.getByText('home-view')).toBeInTheDocument();
    expect(screen.queryByText('editor-view')).not.toBeInTheDocument();
  });
});
