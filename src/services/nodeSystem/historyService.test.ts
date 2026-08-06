import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  HistoryMutationDto,
  MutationRequestDto,
} from '@/shared/types/dto/editorMutation';
import { HistoryService } from './historyService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const locale = 'en-US';
const request: MutationRequestDto<HistoryMutationDto> = {
  resource: { kind: 'graph', key: 'functions/Main.yssbi-function' },
  baseRevision: 5,
  operationId: '00000000-0000-0000-0000-000000000401',
  payload: {},
};

describe('HistoryService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it('sends the captured project identity when reading status', async () => {
    await HistoryService.getStatus(projectInstanceId);

    expect(invoke).toHaveBeenCalledWith('get_project_history_status', { projectInstanceId });
  });

  it.each(['undo', 'redo'] as const)(
    'sends the captured project identity with %s requests',
    async (direction) => {
      await HistoryService[direction](projectInstanceId, locale, request);

      expect(invoke).toHaveBeenCalledWith(`${direction}_graph_document`, {
        projectInstanceId,
        locale,
        request,
      });
    },
  );
});
