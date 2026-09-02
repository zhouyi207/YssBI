import { invokeCommand } from "@/services/ipc";
import {
  isLocalizedCatalogDto,
  type LocalizedCatalogDto,
} from "@/shared/types/dto/localizedCatalog";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { GraphDocumentDto } from "@/shared/types/dto/editorMutation";

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
  document: GraphDocumentDto;
  sourcePort: PortAddressDto;
  locale: string;
}

export class CatalogService {
  static async getCompatibleNodeCatalog(
    request: CompatibleNodeCatalogRequest,
  ): Promise<LocalizedCatalogDto> {
    const response: unknown = await invokeCommand("get_compatible_node_catalog", {
      projectInstanceId: request.projectInstanceId,
      graphPath: request.graphPath,
      document: request.document,
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
