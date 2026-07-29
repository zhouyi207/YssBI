import { existsSync, readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import { deleteResource } from '@/features/application/resource/resourceActions';
import { deleteGraphWithConfirm } from './deleteGraphWithConfirm';

vi.mock('@/features/application/resource/resourceActions', () => ({
  deleteResource: vi.fn(async () => undefined),
}));

describe('deleteGraphWithConfirm backend cascade', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('uses one confirmation and delegates function removal to the canonical backend cascade', async () => {
    const confirm = vi.spyOn(uiStore, 'confirm').mockResolvedValue(true);

    await expect(deleteGraphWithConfirm('functions/Target.yssbi-function', 'function'))
      .resolves.toBe(true);

    expect(confirm).toHaveBeenCalledOnce();
    expect(deleteResource).toHaveBeenCalledWith({
      id: 'functions/Target.yssbi-function',
      kind: 'function',
    });
  });

  it('contains no reachable invokes for removed function call-site commands', () => {
    const files = [
      '../../../services/graph/graphService.ts',
      './deleteGraphWithConfirm.ts',
      '../graphDocument/graphDocumentActions.ts',
    ];
    const source = files
      .map((file) => readFileSync(new URL(file, import.meta.url), 'utf8'))
      .join('\n');

    expect(source).not.toMatch(
      /get_function_call_sites|purge_function_call_sites|update_call_function_target/,
    );
    expect(source).not.toMatch(
      /getFunctionCallSites|purgeFunctionCallSites|updateCallFunctionTarget/,
    );
    expect(
      existsSync(new URL('../graphDocument/useFunctionCallSites.ts', import.meta.url)),
    ).toBe(false);
  });
});
