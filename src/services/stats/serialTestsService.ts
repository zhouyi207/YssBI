import { invokeCommand } from "@/services/ipc";
import {
  normalizeSerialTestsResponse,
  type SerialTestsRequestDTO,
  type SerialTestsResponseDTO,
  type SerialTestWithLagDTO,
  type DurbinWatsonResultDTO,
} from "@/shared/types/report";

export type SerialTestsRequest = SerialTestsRequestDTO;
export type SerialTestsResponse = SerialTestsResponseDTO;
export type SerialTestWithLag = SerialTestWithLagDTO;
export type DurbinWatsonResult = DurbinWatsonResultDTO;

/** 计算 BG、Q、DW 检验 */
export async function computeSerialTests(req: SerialTestsRequest): Promise<SerialTestsResponse> {
  const raw = await invokeCommand<unknown>("compute_serial_tests", { req });
  const parsed = normalizeSerialTestsResponse(raw);
  if (!parsed) {
    throw new Error("序列相关检验：响应格式无效");
  }
  return parsed;
}
