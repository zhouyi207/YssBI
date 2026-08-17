import { invokeCommand } from '@/services/ipc';

export interface PinPreviewGenerationDto {
  generation: number;
}

export function parsePinPreviewGenerationDto(value: unknown): PinPreviewGenerationDto {
  if (
    typeof value !== 'object'
    || value === null
    || Array.isArray(value)
    || Object.keys(value).length !== 1
    || !Object.prototype.hasOwnProperty.call(value, 'generation')
  ) throw new Error('Invalid pin preview generation');
  const generation = (value as Record<string, unknown>).generation;
  if (!Number.isSafeInteger(generation) || (generation as number) <= 0) {
    throw new Error('Invalid pin preview generation');
  }
  return { generation: generation as number };
}

export class PinPreviewGenerationService {
  static async allocate(): Promise<number> {
    const value = await invokeCommand<unknown>('allocate_pin_preview_generation');
    return parsePinPreviewGenerationDto(value).generation;
  }
}
