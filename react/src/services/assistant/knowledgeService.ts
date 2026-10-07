import { z } from "zod";
import { invokeCommand } from "@/services/ipc";
import { harnessResourceRefSchema } from "./harnessContract";

export const knowledgeSourceSchema = z.strictObject({
  sourceId: z.string().min(1),
  title: z.string(),
  path: z.string().min(1),
  status: z.enum(["ready", "changed", "unavailable", "empty"]),
  updatedAt: z.number().int().nonnegative().safe(),
});
export const citationDetailSchema = z.strictObject({
  text: z.string(),
  resource: harnessResourceRefSchema.nullable(),
});
export const knowledgeSourcesSchema = z.array(knowledgeSourceSchema);
export type KnowledgeSource = z.infer<typeof knowledgeSourceSchema>;
export type CitationDetail = z.infer<typeof citationDetailSchema>;

export const KnowledgeService = {
  async list(projectInstanceId: string) {
    return knowledgeSourcesSchema.parse(
      await invokeCommand("list_harness_knowledge", { projectInstanceId }),
    );
  },
  async rebuild(projectInstanceId: string, path: string) {
    await invokeCommand("rebuild_harness_knowledge", { projectInstanceId, path });
  },
  async remove(projectInstanceId: string, sourceId: string) {
    await invokeCommand("remove_harness_knowledge", { projectInstanceId, sourceId });
  },
};
