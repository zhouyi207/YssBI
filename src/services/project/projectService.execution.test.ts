import { Channel, invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RunEvent } from '@/shared/types/dto/runEvent';
import type { ExecutionDemandDto } from '@/shared/types/dto/executionDemand';
import { disposeTrackedChannelsForHmr } from '@/services/devHmrIpc';
import {
  ProjectService,
  isExecutionCancelledError,
} from './projectService';

const projectInstanceId = 'project-instance-1';

vi.mock('@tauri-apps/api/core', () => {
  class TestChannel<T> {
    onmessage?: (message: T) => void;
  }

  return {
    Channel: TestChannel,
    invoke: vi.fn(),
  };
});

function runEvent(kind: RunEvent['kind']): RunEvent {
  return {
    correlation: {
      projectSessionId: 'project-session-1',
      graphPath: 'events/Main.yssbi-event',
      graphRevision: '7',
      registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
      resourceVersions: {},
      compileId: '9',
      selectionDigest: 'demand-selection-a',
      runId: '41',
      nodeId: null,
      nodeTypeId: null,
      parentCall: null,
    },
    basis: {
      graphRevision: '7',
      registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
      resourceVersions: {},
    },
    kind,
  };
}

describe('ProjectService execution contract', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes cancel_graph_run with an opaque decimal run ID', async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    await expect(ProjectService.cancelGraphRun('9007199254740993')).resolves.toBe(true);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith('cancel_graph_run', {
      runId: '9007199254740993',
    });
  });

  it('classifies only the structured canonical run cancellation error', () => {
    expect(isExecutionCancelledError({
      code: 'run_cancelled',
      message: 'run was cancelled',
    })).toBe(true);
    expect(isExecutionCancelledError('EXECUTION_CANCELLED')).toBe(false);
  });

  it.each([
    {
      name: 'declared output',
      demand: {
        type: 'outputs' as const,
        outputs: [{
          graphPath: 'events/Main.yssbi-event',
          port: {
            kind: 'declared' as const,
            nodeId: '00000000-0000-0000-0000-000000000002',
            portKey: 'result',
          },
        }],
        includeDefaultResults: false,
      },
    },
    {
      name: 'dynamic output instance',
      demand: {
        type: 'outputs' as const,
        outputs: [{
          graphPath: 'events/Main.yssbi-event',
          port: {
            kind: 'instance' as const,
            nodeId: '00000000-0000-0000-0000-000000000002',
            templateKey: 'value',
            instanceId: '00000000-0000-0000-0000-000000000003',
          },
        }],
        includeDefaultResults: false,
      },
    },
  ])('forwards the exact $name demand', async ({ demand }) => {
    const commandError = { code: 'test_stop', message: 'stop after invoke' };
    vi.mocked(invoke).mockRejectedValue(commandError);

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      demand,
    );

    await expect(execution).rejects.toBe(commandError);
    expect(vi.mocked(invoke).mock.calls[0]?.[1]).toMatchObject({ demand });
  });

  it('rejects a malformed demand before invoking execute_graph_document', async () => {
    const malformed = { type: 'default', extra: true } as unknown as ExecutionDemandDto;

    await expect(ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      malformed,
    )).rejects.toThrow('Invalid default execution demand');

    expect(invoke).not.toHaveBeenCalled();
  });

  it('rejects a malformed execute result immediately after invoke', async () => {
    vi.mocked(invoke).mockResolvedValue({ runId: 41 });
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { onEvent: Channel<RunEvent> },
    ];
    args.onEvent.onmessage?.(runEvent({ type: 'runCompleted' }));

    await expect(execution).rejects.toThrow('Invalid execute graph result');
  });

  it('invokes execute_graph_document and drains its RunEvent channel before resolving', async () => {
    let resolveInvoke!: (result: { runId: string }) => void;
    vi.mocked(invoke).mockReturnValue(new Promise((resolve) => {
      resolveInvoke = resolve;
    }));
    const received: RunEvent[] = [];

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      (event) => received.push(event),
    );

    expect(invoke).toHaveBeenCalledOnce();
    const [command, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; demand: { type: 'default' }; onEvent: Channel<RunEvent> },
    ];
    expect(command).toBe('execute_graph_document');
    expect(args).toEqual({
      projectInstanceId,
      graphPath: 'events/Main.yssbi-event',
      demand: { type: 'default' },
      onEvent: expect.any(Channel),
    });

    resolveInvoke({ runId: '41' });
    let settled = false;
    void execution.then(() => {
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);

    const completed: RunEvent = {
      correlation: {
        projectSessionId: 'project-session-1',
        graphPath: 'events/Main.yssbi-event',
        graphRevision: '7',
        registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
        resourceVersions: {},
        compileId: '9',
        selectionDigest: 'demand-selection-a',
        runId: '41',
        nodeId: null,
        nodeTypeId: null,
        parentCall: null,
      },
      basis: {
        graphRevision: '7',
        registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
        resourceVersions: {},
      },
      kind: { type: 'runCompleted' },
    };
    args.onEvent.onmessage?.(completed);

    await expect(execution).resolves.toEqual({ runId: '41' });
    expect(received).toEqual([completed]);
  });

  it('surfaces a throwing runCompleted consumer after a successful invoke settles', async () => {
    const consumerError = new Error('runCompleted consumer failed');
    vi.mocked(invoke).mockResolvedValue({ runId: '41' });
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      () => { throw consumerError; },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];

    expect(() => args.onEvent.onmessage?.(runEvent({ type: 'runCompleted' }))).not.toThrow();

    await expect(execution).rejects.toBe(consumerError);
  });

  it.each([
    {
      terminal: { type: 'runErrored', code: 'kernelFailed' } as const,
      commandError: {
        code: 'run_failed',
        message: 'run failed',
        details: { terminalRunEventSent: true },
      },
    },
    {
      terminal: { type: 'runCancelled' } as const,
      commandError: {
        code: 'run_cancelled',
        message: 'run was cancelled',
        details: { terminalRunEventSent: true },
      },
    },
  ])('preserves the backend rejection when the $terminal.type consumer throws', async ({
    terminal,
    commandError,
  }) => {
    const consumerError = new Error(`${terminal.type} consumer failed`);
    vi.mocked(invoke).mockRejectedValue(commandError);
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      () => { throw consumerError; },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];

    expect(() => args.onEvent.onmessage?.(runEvent(terminal))).not.toThrow();

    await expect(execution).rejects.toBe(commandError);
  });

  it('rejects its pending drain when HMR disposes the execution channel', async () => {
    vi.mocked(invoke).mockResolvedValue({ runId: '41' });
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];
    const outcome = execution.then(
      () => 'resolved' as const,
      (error: unknown) => error,
    );

    disposeTrackedChannelsForHmr();
    const observed = await Promise.race([
      outcome,
      new Promise<'timeout'>((resolve) => setTimeout(() => resolve('timeout'), 10)),
    ]);
    if (observed === 'timeout') {
      args.onEvent.onmessage?.({
        correlation: {
          projectSessionId: 'project-session-1',
          graphPath: 'events/Main.yssbi-event',
          graphRevision: '7',
          registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
          resourceVersions: {},
          compileId: '9',
          selectionDigest: 'demand-selection-a',
          runId: '41',
          nodeId: null,
          nodeTypeId: null,
          parentCall: null,
        },
        basis: {
          graphRevision: '7',
          registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
          resourceVersions: {},
        },
        kind: { type: 'runCompleted' },
      });
      await execution;
    }

    expect(observed).toMatchObject({ code: 'execution_channel_disposed' });
  });

  it('preserves the command error when HMR disposes its pending terminal drain', async () => {
    const commandError = {
      code: 'run_cancelled',
      message: 'run was cancelled',
      details: { terminalRunEventSent: true },
    };
    vi.mocked(invoke).mockRejectedValue(commandError);

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    );
    disposeTrackedChannelsForHmr();

    await expect(execution).rejects.toBe(commandError);
  });

  it('drains a terminal run event before rethrowing the original command error', async () => {
    const commandError = {
      code: 'run_failed',
      message: 'run failed',
      details: { terminalRunEventSent: true },
    };
    vi.mocked(invoke).mockRejectedValue(commandError);
    const received: RunEvent[] = [];

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      (event) => received.push(event),
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];

    let settled = false;
    void execution.catch(() => {
      settled = true;
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);

    const errored: RunEvent = {
      correlation: {
        projectSessionId: 'project-session-1',
        graphPath: 'events/Main.yssbi-event',
        graphRevision: '7',
        registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
        resourceVersions: {},
        compileId: '9',
        selectionDigest: 'demand-selection-a',
        runId: '41',
        nodeId: null,
        nodeTypeId: null,
        parentCall: null,
      },
      basis: {
        graphRevision: '7',
        registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
        resourceVersions: {},
      },
      kind: { type: 'runErrored', code: 'kernelFailed' },
    };
    args.onEvent.onmessage?.(errored);

    await expect(execution).rejects.toBe(commandError);
    expect(received).toEqual([errored]);
  });
});
