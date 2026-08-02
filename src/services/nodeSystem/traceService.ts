import { invoke } from '@tauri-apps/api/core';
import type { TraceDecimalString, TraceRecordDto } from '@/shared/types/dto/trace';

export class TraceService {
  static async listGraphTraces(
    projectInstanceId: string,
    graphPath: string,
  ): Promise<TraceRecordDto[]> {
    return invoke<TraceRecordDto[]>('list_graph_traces', {
      projectInstanceId,
      graphPath,
    });
  }

  static async getRunTrace(
    projectInstanceId: string,
    runId: TraceDecimalString,
  ): Promise<TraceRecordDto[]> {
    return invoke<TraceRecordDto[]>('get_run_trace', {
      projectInstanceId,
      runId,
    });
  }
}
