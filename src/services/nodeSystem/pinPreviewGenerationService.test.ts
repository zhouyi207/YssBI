import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  parsePinPreviewGenerationDto,
  PinPreviewGenerationService,
} from './pinPreviewGenerationService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('PinPreviewGenerationService', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it('allocates through the project-independent command without arguments', async () => {
    vi.mocked(invoke).mockResolvedValue({ generation: 17 });

    await expect(PinPreviewGenerationService.allocate()).resolves.toBe(17);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith('allocate_pin_preview_generation');
  });

  it('accepts only an exact positive safe-integer generation DTO', () => {
    expect(parsePinPreviewGenerationDto({ generation: 1 })).toEqual({ generation: 1 });
    expect(parsePinPreviewGenerationDto({ generation: Number.MAX_SAFE_INTEGER })).toEqual({
      generation: Number.MAX_SAFE_INTEGER,
    });
  });

  it.each([
    null,
    [],
    {},
    { generation: 1, extra: true },
    { generation: 0 },
    { generation: -1 },
    { generation: 1.5 },
    { generation: Number.MAX_SAFE_INTEGER + 1 },
    { generation: '1' },
  ])('rejects malformed generation DTO %#', (value) => {
    expect(() => parsePinPreviewGenerationDto(value)).toThrow('Invalid pin preview generation');
  });
});
