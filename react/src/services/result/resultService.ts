import { invokeCommand } from "@/services/ipc";
import type {
  ResultTablePart,
  ResultAnalysisRequest,
  ResultAnalysis,
} from "@/shared/types/domain/resultReport";
import {
  resultReference,
  resultReferenceKey,
  type ResultReference,
  type ResultLease,
  type GraphResultState,
} from "@/shared/types/domain/result";
import { parseResultAnalysis } from "@/shared/types/report/parseLinearRegression";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import { portAddressKey } from "@/shared/types/domain/portAddressKey";
import type { ResultDescriptor, ResultPage, ResultValue } from "@/shared/types/dto/result";
import {
  parseResultDescriptor,
  parseResultPage,
  parseResultValue,
  parseResultLease,
  parseGraphResultState,
} from "@/shared/types/dto/resultParser";

function nullable<T>(value: unknown, parse: (input: unknown) => T): T | null {
  return value === null ? null : parse(value);
}

export class ResultService {
  static async getGraphState(
    graphPath: string,
    semanticInputHash: string,
  ): Promise<GraphResultState | null> {
    const value = nullable(
      await invokeCommand<unknown>("get_graph_result_state", { graphPath, semanticInputHash }),
      parseGraphResultState,
    );
    if (
      value &&
      (value.semanticInputHash !== semanticInputHash ||
        [...value.outputs, ...value.connections].some(
          ({ output }) => output.graphPath !== graphPath,
        ))
    )
      throw new Error("Mismatched graph result state");
    return value;
  }

  static async getDescriptor(reference: ResultReference): Promise<ResultDescriptor | null> {
    const expected = resultReference(reference);
    const value = await invokeCommand<unknown>("get_result_descriptor", {
      reference: expected,
    });
    const descriptor = nullable(value, parseResultDescriptor);
    if (descriptor && resultReferenceKey(descriptor) !== resultReferenceKey(expected))
      throw new Error("Mismatched result descriptor");
    return descriptor;
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
    const expected = resultReference(reference);
    const value = part
      ? await invokeCommand<unknown>("get_result_table_page", {
          reference: expected,
          part,
          offset,
          limit,
        })
      : await invokeCommand<unknown>("get_result_page", {
          reference: expected,
          offset,
          limit,
        });
    const page = nullable(value, parseResultPage);
    if (
      page &&
      (page.resultId !== expected.resultId ||
        page.requestedLimit !== limit ||
        page.offset !== (page.totalCount === null ? offset : Math.min(offset, page.totalCount)))
    )
      throw new Error("Mismatched result page");
    return page;
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
    const lease = parseResultLease(
      await invokeCommand<unknown>("claim_result_lease", { lease: leaseId }),
    );
    if (lease.leaseId !== leaseId) throw new Error("Mismatched result lease");
    return lease;
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
    const expectedOutput = portAddressKey(output);
    const value = await invokeCommand<unknown>("get_pin_result", { graphPath, output });
    const descriptor = nullable(value, parseResultDescriptor);
    if (descriptor) {
      const source = descriptor.provenance.output;
      if (
        !source ||
        source.graphPath !== graphPath ||
        portAddressKey(source.port) !== expectedOutput
      )
        throw new Error("Mismatched result output");
    }
    return descriptor;
  }
}
