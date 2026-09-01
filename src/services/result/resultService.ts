import { invokeCommand } from "@/services/ipc";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type {
  PinResultEntry,
  ResultDescriptor,
  ResultPage,
  ResultValue,
} from "@/shared/types/dto/result";
import {
  parsePinResultHistory,
  parseResultDescriptor,
  parseResultPage,
  parseResultValue,
} from "@/shared/types/dto/resultParser";

function nullable<T>(value: unknown, parse: (input: unknown) => T): T | null {
  return value === null ? null : parse(value);
}

export class ResultService {
  static async getDescriptor(resultId: string): Promise<ResultDescriptor | null> {
    const value = await invokeCommand<unknown>("get_result_descriptor", { resultId });
    return nullable(value, parseResultDescriptor);
  }

  static async getValue(resultId: string): Promise<ResultValue | null> {
    const value = await invokeCommand<unknown>("get_result_value", { resultId });
    return nullable(value, parseResultValue);
  }

  static async getPage(
    resultId: string,
    offset: number,
    limit: number,
  ): Promise<ResultPage | null> {
    const value = await invokeCommand<unknown>("get_result_page", { resultId, offset, limit });
    return nullable(value, parseResultPage);
  }

  static async getPinHistory(graphPath: string, output: PortAddressDto): Promise<PinResultEntry[]> {
    const value = await invokeCommand<unknown>("get_pin_result_history", { graphPath, output });
    return parsePinResultHistory(value);
  }
}
