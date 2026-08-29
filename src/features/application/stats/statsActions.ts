import {
  computeAcfPacf as computeAcfPacfService,
  type AcfPacfRequest as ServiceAcfPacfRequest,
} from '@/services/stats/acfPacfService';
import {
  computeSerialTests as computeSerialTestsService,
  type SerialTestsRequest as ServiceSerialTestsRequest,
} from '@/services/stats/serialTestsService';
import { hypothesisTest as hypothesisTestService } from '@/services/stats/hypothesisService';
import { parseAtValues as parseAtValuesService } from '@/services/stats/parseAtService';
import { PanelDidService } from '@/services/stats/panelDidService';

export { PanelDidService };

export interface AcfPacfRequest extends ServiceAcfPacfRequest {}

export interface AcfPacfResponse {
  readonly acf: number[];
  readonly pacf: number[];
  readonly n: number;
}

export async function computeAcfPacf(req: AcfPacfRequest): Promise<AcfPacfResponse> {
  return computeAcfPacfService(req);
}

export interface ParseAtRequest {
  readonly param_names: string[];
  readonly at_spec: string;
}

export interface ParseAtResponse {
  readonly values: Record<string, number>;
}

export async function parseAtValues(req: ParseAtRequest): Promise<ParseAtResponse> {
  return parseAtValuesService(req);
}

export interface SerialTestWithLag {
  readonly stat: number;
  readonly p_value: number;
  readonly lags: number;
}

export interface DurbinWatsonResult {
  readonly d: number;
}

export interface SerialTestsResponse {
  readonly bg?: SerialTestWithLag;
  readonly q?: SerialTestWithLag;
  readonly dw: DurbinWatsonResult;
}

export interface SerialTestsRequest extends ServiceSerialTestsRequest {}

export async function computeSerialTests(
  req: SerialTestsRequest,
): Promise<SerialTestsResponse> {
  return computeSerialTestsService(req);
}

export {
  hypothesisTestService as hypothesisTest,
};
export { useRegressionReport } from './useRegressionReport';
export { useHypothesisTestBlock, useHypothesisTestBlock as useStatsBlock } from './useHypothesisTestBlock';
export { useDidFakeGroupRi } from './useDidFakeGroupRi';
export type { HypothesisTestResponse } from '@/services/stats/hypothesisService';
