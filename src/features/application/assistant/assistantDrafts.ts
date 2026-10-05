import { languageModelSelectionSchema } from "@/services/assistant/modelContract";
import { z } from "zod";
import { harnessResourceRefSchema } from "@/services/assistant/harnessContract";
import type { ResourceRef } from "@/shared/types/domain/resource";

const pendingSchema = z.object({
  text: z.string(),
  resources: z.array(harnessResourceRefSchema),
  model: languageModelSelectionSchema.nullable(),
  afterSequence: z.number().int().nonnegative(),
});
const queuedSchema = z.object({
  id: z.string(),
  text: z.string(),
  resources: z.array(harnessResourceRefSchema),
  model: languageModelSelectionSchema.nullable(),
});
const draftsSchema = z.object({
  pending: pendingSchema.nullable(),
  queued: z.array(queuedSchema),
  resources: z.array(harnessResourceRefSchema),
});
export type PendingAssistantMessage = z.infer<typeof pendingSchema>;
export type QueuedAssistantMessage = z.infer<typeof queuedSchema>;

/** Only unsubmitted inputs live here. Committed chat history always comes from Rust. */
export function readAssistantDrafts(sessionId: string): z.infer<typeof draftsSchema> {
  try {
    const parsed = draftsSchema.safeParse(
      JSON.parse(localStorage.getItem(`yssbi.assistant.pending.${sessionId}`) ?? "null"),
    );
    if (parsed.success) return parsed.data;
  } catch {
    /* Unavailable storage must not prevent reading durable conversations. */
  }
  return { pending: null, queued: [], resources: [] };
}

export function writeAssistantDrafts(
  sessionId: string,
  pending: PendingAssistantMessage | null,
  queued: readonly QueuedAssistantMessage[],
  resources: readonly ResourceRef[],
): void {
  try {
    const key = `yssbi.assistant.pending.${sessionId}`;
    if (!pending && queued.length === 0 && resources.length === 0) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify({ pending, queued, resources }));
  } catch {
    /* The active input remains in the session projection if storage is unavailable. */
  }
}
