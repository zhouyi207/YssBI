import { openExternal } from '@/services/platform/opener';

export async function openExternalUrl(url: string): Promise<void> {
  const result = await openExternal(url);
  if (!result.ok) throw new Error(result.failure.code);
}
