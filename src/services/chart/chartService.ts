import { invokeCommand } from "@/services/ipc";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type { PlotColumnPairPayload } from "@/shared/types/domain/chart";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";

export class ChartService {
  static async createChart(
    projectInstanceId: string,
    operationId: string,
    name: string,
    databaseId?: string,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(
      await invokeCommand<unknown>("create_chart", {
        projectInstanceId,
        operationId,
        name,
        databaseId,
      }),
    );
  }

  static async duplicateChart(
    projectInstanceId: string,
    operationId: string,
    chartPath: string,
    expectedRevision: number,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(
      await invokeCommand<unknown>("duplicate_chart", {
        projectInstanceId,
        operationId,
        chartPath,
        expectedRevision,
      }),
    );
  }

  static async loadChart(projectInstanceId: string, chartPath: string): Promise<ChartDocument> {
    return await invokeCommand("load_chart", { projectInstanceId, chartPath });
  }

  static async saveChart(
    projectInstanceId: string,
    operationId: string,
    chartPath: string,
    expectedRevision: number,
    document: ChartDocument,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(
      await invokeCommand<unknown>("save_chart", {
        projectInstanceId,
        operationId,
        chartPath,
        expectedRevision,
        document,
      }),
    );
  }

  static async renameChart(
    projectInstanceId: string,
    operationId: string,
    chartPath: string,
    expectedRevision: number,
    newName: string,
    lifecycleToken: number,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(
      await invokeCommand<unknown>("rename_chart_resource", {
        projectInstanceId,
        operationId,
        chartPath,
        expectedRevision,
        newName,
        lifecycleToken,
      }),
    );
  }

  static async removeChart(
    projectInstanceId: string,
    operationId: string,
    chartPath: string,
    expectedRevision: number,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(
      await invokeCommand<unknown>("remove_chart", {
        projectInstanceId,
        operationId,
        chartPath,
        expectedRevision,
      }),
    );
  }

  static async getPlotColumnPair(
    projectInstanceId: string,
    databaseId: string,
    xCol: string,
    yCol: string,
    maxPoints?: number,
  ): Promise<PlotColumnPairPayload> {
    return await invokeCommand("get_plot_column_pair", {
      projectInstanceId,
      databaseId,
      xCol,
      yCol,
      maxPoints,
    });
  }
}
