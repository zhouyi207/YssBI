import { invokeCommand } from "@/services/ipc";
import type {
  ResultTablePart,
  ResultAnalysisRequest,
  ResultAnalysis,
} from "@/shared/types/domain/resultReport";
import {
  resultReference,
  type ResultReference,
  type ResultLease,
} from "@/shared/types/domain/result";
import { parseResultAnalysis } from "@/shared/types/report/parseOls";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { ResultDescriptor, ResultPage, ResultValue } from "@/shared/types/dto/result";
import {
  parseResultDescriptor,
  parseResultPage,
  parseResultValue,
  parseResultLease,
} from "@/shared/types/dto/resultParser";

function nullable<T>(value: unknown, parse: (input: unknown) => T): T | null {
  return value === null ? null : parse(value);
}

export class ResultService {
  static async getDescriptor(reference: ResultReference): Promise<ResultDescriptor | null> {
    const value = await invokeCommand<unknown>("get_result_descriptor", {
      reference: resultReference(reference),
    });
    return nullable(value, parseResultDescriptor);
  }

  static async getValue(reference: ResultReference): Promise<ResultValue | null> {
    const value = await invokeCommand<unknown>("get_result_value", {
      reference: resultReference(reference),
    });
    return nullable(value, parseResultValue);
  }

  static async getPage(
    reference: ResultReference,
    offset: number,
    limit: number,
    part?: ResultTablePart,
  ): Promise<ResultPage | null> {
    const value = part
      ? await invokeCommand<unknown>("get_result_table_page", {
          reference: resultReference(reference),
          part,
          offset,
          limit,
        })
      : await invokeCommand<unknown>("get_result_page", {
          reference: resultReference(reference),
          offset,
          limit,
        });
    return nullable(value, parseResultPage);
  }

  static async retain(
    reference: ResultReference,
    leaseId: string,
    handoff?: string,
  ): Promise<ResultLease> {
    return parseResultLease(
      await invokeCommand<unknown>("retain_result", {
        reference: resultReference(reference),
        lease: leaseId,
        handoff: handoff ?? null,
      }),
    );
  }

  static async claim(leaseId: string): Promise<ResultLease> {
    return parseResultLease(await invokeCommand<unknown>("claim_result_lease", { lease: leaseId }));
  }

  static async release(leaseId: string): Promise<void> {
    await invokeCommand("release_result_lease", { lease: leaseId });
  }

  static async reconcileLeases(leaseIds: readonly string[]): Promise<void> {
    await invokeCommand("reconcile_result_leases", { leases: leaseIds });
  }

  static async analyze(
    reference: ResultReference,
    analysis: ResultAnalysisRequest,
  ): Promise<ResultAnalysis> {
    const value = parseResultAnalysis(
      await invokeCommand<unknown>("analyze_result", { reference, analysis }),
    );
    if (value.kind !== analysis.kind) throw new Error("Mismatched result analysis");
    return value;
  }

  static async getPinResult(
    graphPath: string,
    output: PortAddressDto,
  ): Promise<ResultDescriptor | null> {
    const value = await invokeCommand<unknown>("get_pin_result", { graphPath, output });
    return nullable(value, parseResultDescriptor);
  }
}
