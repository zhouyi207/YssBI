import { invokeCommand } from "@/services/ipc";
import { z } from "zod";
import { isJsonValue, isParameterGroups } from "@/shared/types/domain/editorProjectionGuards";
import type { ParameterGroupDto } from "@/shared/types/domain/editorProjection";
import {
  isLocalizedCatalogDto,
  type LocalizedCatalogDto,
} from "@/shared/types/dto/localizedCatalog";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { GraphEditVersionDto } from "@/shared/types/dto/editorMutation";

export type {
  LocalizedCatalogDto,
  LocalizedCatalogItemDto,
  LocalizedCategoryDto,
  LocalizedParameterDto,
  LocalizedPortDto,
} from "@/shared/types/dto/localizedCatalog";

export interface CompatibleNodeCatalogRequest {
  projectInstanceId: string;
  graphPath: string;
  version: GraphEditVersionDto;
  sourcePort: PortAddressDto;
  locale: string;
}

const parameterFormSchema = z
  .object({
    values: z.record(z.string(), z.custom(isJsonValue)),
    groups: z.custom<ParameterGroupDto[]>(isParameterGroups),
    portCounts: z.record(z.string(), z.number().int().min(0).max(65535)),
    ports: z.array(
      z
        .object({
          key: z.string().min(1),
          title: z.string(),
          direction: z.enum(["input", "output"]),
          count: z.discriminatedUnion("kind", [
            z.object({ kind: z.literal("fixed") }).strict(),
            z.object({ kind: z.literal("derived") }).strict(),
            z
              .object({
                kind: z.literal("configurable"),
                min: z.number().int().min(0).max(65535),
                max: z.number().int().min(0).max(65535).nullable(),
                memberTemplates: z.array(z.string().min(1)).min(1),
              })
              .strict(),
          ]),
        })
        .strict(),
    ),
  })
  .strict();

export type NodeCreationForm = z.infer<typeof parameterFormSchema>;

export class CatalogService {
  static async getNodeCreationForm(
    projectInstanceId: string,
    nodeTypeId: string,
    parameters: Record<string, unknown>,
    portCounts: Record<string, number>,
    locale: string,
  ): Promise<NodeCreationForm> {
    return parameterFormSchema.parse(
      await invokeCommand("get_node_creation_form", {
        projectInstanceId,
        nodeTypeId,
        parameters,
        portCounts,
        locale,
      }),
    );
  }

  static async getCompatibleNodeCatalog(
    request: CompatibleNodeCatalogRequest,
  ): Promise<LocalizedCatalogDto> {
    const response: unknown = await invokeCommand("get_compatible_node_catalog", {
      projectInstanceId: request.projectInstanceId,
      graphPath: request.graphPath,
      version: request.version,
      sourcePort: request.sourcePort,
      locale: request.locale,
    });
    if (!isLocalizedCatalogDto(response)) {
      throw new Error("Invalid compatible node catalog response");
    }
    return response;
  }

  static async getLocalizedCatalog(
    projectInstanceId: string,
    locale: string,
  ): Promise<LocalizedCatalogDto> {
    const response: unknown = await invokeCommand("get_localized_node_catalog", {
      projectInstanceId,
      locale,
    });
    if (!isLocalizedCatalogDto(response)) {
      throw new Error("Invalid localized node catalog response");
    }
    return response;
  }
}
