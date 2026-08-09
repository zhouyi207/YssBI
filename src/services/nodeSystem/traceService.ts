import { invoke } from '@tauri-apps/api/core';
import {
  parseTraceSpanList,
  type TraceDecimalString,
  type TraceSpanDto,
} from '@/shared/types/dto/trace';

export class TraceService {
  static async listGraphTraces(
    projectInstanceId: string,
    graphPath: string,
  ): Promise<TraceSpanDto[]> {
    const response: unknown = await invoke('list_graph_traces', {
      projectInstanceId,
      graphPath,
    });
    return parseTraceSpanList(response);
  }

  static async getRunTrace(
    projectInstanceId: string,
    runId: TraceDecimalString,
  ): Promise<TraceSpanDto[]> {
    const response: unknown = await invoke('get_run_trace', {
      projectInstanceId,
      runId,
    });
    return parseTraceSpanList(response);
  }
}
