import {
  applyGraphMutation,
  enqueueGraphTask,
} from "@/features/application/graphEditing/graphEditCoordinator";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { installGraphRunEvent } from "@/features/application/editor/observeGraphRunEvent";
import { recoverGraphExecution } from "@/features/application/graphProjection/graphActivity";
import { ProjectService } from "@/services/project/projectService";
import { ResultService } from "@/services/result/resultService";
import { resultReferenceKey, type ResultReference } from "@/shared/types/domain/result";
import type { LinearSummaryAddition } from "@/shared/types/domain/resultReport";
import { linearRegressionReportField } from "@/shared/types/report/parseLinearRegression";
import { resultLeases } from "./resultLeases";
import { resultQueryCoordinator, resultQueryRead } from "./runtime";

const unavailable = () => ({ code: "report_summary_unavailable", incidentId: null });
const changed = () => ({ code: "report_summary_changed", incidentId: null });

/** Explicit report editing shares the node's normal parameter transaction and execution path. */
export async function addLinearSummaryContents(
  reference: ResultReference,
  additions: LinearSummaryAddition,
  isActive: () => boolean,
) {
  const project = captureProjectIdentity();
  const isCurrent = () => isActive() && isCurrentProjectIdentity(project);
  const descriptor = await ResultService.getDescriptor(reference);
  if (!isCurrent()) throw changed();
  const source = descriptor?.provenance.output;
  if (
    !source ||
    descriptor?.presentation.kind !== "report" ||
    descriptor.presentation.report !== "linearRegressionSummary"
  )
    throw unavailable();
  const graphPath = source.graphPath;
  if (!(await useProjectIOStore.getState().loadGraph(graphPath)) || !isCurrent())
    throw unavailable();
  const outcome = await applyGraphMutation({
    graphPath,
    mutation(document) {
      if (!isCurrent()) throw changed();
      const node = document.nodes[descriptor.provenance.nodeId];
      if (node?.node_type !== "yssbi.statistics.linear.summary") throw unavailable();
      // setParameters merges these additions into the current Rust-owned values.
      return { type: "setParameters", payload: { nodeId: node.id, parameters: additions } };
    },
  });
  if (!isCurrent()) throw changed();
  if (outcome.status !== "applied" && outcome.status !== "noop")
    throw outcome.status === "rejected" ? { code: outcome.code, incidentId: null } : changed();
  const version = outcome.result.editing.version;
  const currentVersion = () => {
    const current = useGraphProjectionStore.getState().sessions[graphPath];
    return (
      isCurrent() &&
      current?.version.sessionId === version.sessionId &&
      current.version.revision === version.revision
    );
  };
  let completedSessionId: string | undefined;
  const dispatched = await enqueueGraphTask(
    graphPath,
    async () => {
      if (!currentVersion()) throw changed();
      const draft = useGraphProjectionStore.getState().sessions[graphPath];
      if (draft.saving) throw changed();
      return {
        completion: ProjectService.executeGraph({
          projectInstanceId: project.projectInstanceId,
          graphPath,
          version,
          semanticInputHash: draft.semanticInputHash,
          demand: {
            type: "outputs",
            outputs: [source],
            reuseInputs: true,
            includeDefaultResults: false,
          },
          onEvent(event) {
            if (!isCurrent()) return;
            if (event.kind.type !== "resultInspectionRequested") installGraphRunEvent(event);
            if (event.kind.type === "runCompleted")
              completedSessionId = event.run.executionSessionId;
          },
        }),
      };
    },
    null,
  );
  if (!dispatched) throw changed();
  try {
    await dispatched.completion;
  } finally {
    if (isCurrent()) await recoverGraphExecution(project).catch(() => undefined);
  }
  if (!currentVersion() || !completedSessionId) throw changed();
  const result = await ResultService.getPinResult(graphPath, source.port);
  // This query revalidates the current output's inputs, including concurrent runs.
  if (!currentVersion() || !result || result.executionSessionId !== completedSessionId)
    throw changed();
  const held = await resultLeases.acquire(result);
  const releasePayload = resultQueryCoordinator.retainPayload(result);
  let released = false;
  const finish = (installed: boolean) => {
    if (released) return;
    released = true;
    releasePayload();
    void resultLeases.finish(held.leaseId, installed);
  };
  const release = () => finish(false);
  try {
    const loaded = await resultQueryCoordinator.loadValue(result);
    if (!currentVersion()) throw changed();
    const value = resultQueryRead.getValue(result);
    if (loaded.status !== "published" || value?.kind !== "value") throw unavailable();
    const parsed = linearRegressionReportField.read(value.value, "$");
    if (!parsed.ok || resultReferenceKey(parsed.value.resultRef) !== resultReferenceKey(result))
      throw unavailable();
    return { data: parsed.value, lease: held, release, transferToPanel: () => finish(true) };
  } catch (error) {
    release();
    throw error;
  }
}
