import { invokeCommand } from '@/services/ipc';
import {
  parseRunTraceBundle,
  parseTraceBundleList,
  type RunTraceBundleDto,
  type TraceBundleDto,
  type TraceDecimalString,
} from '@/shared/types/dto/trace';

export class TraceService {
  static async listGraphTraceBundles(
    projectInstanceId: string,
    graphPath: string,
  ): Promise<TraceBundleDto[]> {
    const response: unknown = await invokeCommand('list_graph_trace_bundles', {
      projectInstanceId,
      graphPath,
    });
    return parseTraceBundleList(response);
  }

  static async getRunTraceBundle(
    projectInstanceId: string,
    runId: TraceDecimalString,
  ): Promise<RunTraceBundleDto> {
    const response: unknown = await invokeCommand('get_run_trace_bundle', {
      projectInstanceId,
      runId,
    });
    return parseRunTraceBundle(response);
  }
}
