import { describe, expect, it } from 'vitest';
import {
  applyExecutionVisualEvent,
  connectionKey,
  getExecutionVisual,
  resetExecutionVisual,
  clearExecutionVisual,
  snapshotToGraphPatch,
} from './executionVisualSession';

describe('executionVisualSession', () => {
  it('tracks node and connection lifecycle', () => {
    resetExecutionVisual('g1');
    applyExecutionVisualEvent('g1', { event: 'nodeStart', data: { nodeId: 'n1' } });
    expect(getExecutionVisual().executingNodeId).toBe('n1');

    applyExecutionVisualEvent('g1', {
      event: 'connectionActive',
      data: { fromPinId: 'p1', toPinId: 'p2' },
    });
    expect(getExecutionVisual().completedConnections.has(connectionKey('p1', 'p2'))).toBe(true);
    expect(getExecutionVisual().flowingConnections.has(connectionKey('p1', 'p2'))).toBe(false);

    applyExecutionVisualEvent('g1', {
      event: 'connectionFlow',
      data: { fromPinId: 'p1', toPinId: 'p2' },
    });
    expect(getExecutionVisual().flowingConnections.has(connectionKey('p1', 'p2'))).toBe(true);

    applyExecutionVisualEvent('g1', { event: 'nodeComplete', data: { nodeId: 'n1', durationMs: 12 } });
    const snap = getExecutionVisual();
    expect(snap.executingNodeId).toBeNull();
    expect(snap.executedNodeIds.has('n1')).toBe(true);
    expect(snap.nodeDurations.get('n1')).toBe(12);

    applyExecutionVisualEvent('g1', { event: 'executionComplete', data: { hasError: false } });
    const patch = snapshotToGraphPatch(getExecutionVisual());
    expect(patch.status).toBe('completed');
    expect(patch.nodeStates.get('n1')?.status).toBe('completed');
    expect(patch.nodeStates.get('n1')?.durationMs).toBe(12);
    expect(patch.flowingConnections.has(connectionKey('p1', 'p2'))).toBe(true);

    clearExecutionVisual();
    expect(getExecutionVisual().active).toBe(false);
  });

  it('nodeError marks node but keeps running until executionComplete', () => {
    resetExecutionVisual('g1');
    applyExecutionVisualEvent('g1', { event: 'nodeError', data: { nodeId: 'bad', error: 'boom' } });
    expect(getExecutionVisual().status).toBe('running');
    expect(getExecutionVisual().errorNodeIds.has('bad')).toBe(true);

    applyExecutionVisualEvent('g1', { event: 'executionComplete', data: { hasError: true } });
    expect(getExecutionVisual().status).toBe('error');
  });
});
