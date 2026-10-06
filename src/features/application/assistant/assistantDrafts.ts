import { languageModelSelectionSchema } from "@/services/assistant/modelContract";
import { z } from "zod";
import {
  harnessResourceRefSchema,
  harnessTurnOptionsSchema,
  DEFAULT_TURN_OPTIONS,
  type HarnessTurnOptions,
} from "@/services/assistant/harnessContract";
import type { ResourceRef } from "@/shared/types/domain/resource";

const pendingSchema = z.object({
  options: harnessTurnOptionsSchema,
  text: z.string(),
  resources: z.array(harnessResourceRefSchema),
  model: languageModelSelectionSchema.nullable(),
  afterSequence: z.number().int().nonnegative(),
});
const queuedSchema = z.object({
  options: harnessTurnOptionsSchema,
  id: z.string(),
  text: z.string(),
  resources: z.array(harnessResourceRefSchema),
  model: languageModelSelectionSchema.nullable(),
});
const draftsSchema = z.object({
  options: harnessTurnOptionsSchema,
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
  return { pending: null, queued: [], resources: [], options: DEFAULT_TURN_OPTIONS };
}

export function writeAssistantDrafts(
  sessionId: string,
  pending: PendingAssistantMessage | null,
  queued: readonly QueuedAssistantMessage[],
  resources: readonly ResourceRef[],
  options: HarnessTurnOptions,
): void {
  try {
    const key = `yssbi.assistant.pending.${sessionId}`;
    localStorage.setItem(key, JSON.stringify({ pending, queued, resources, options }));
  } catch {
    /* The active input remains in the session projection if storage is unavailable. */
  }
}
