import { useEffect } from 'react';

import { loadActivatedProject, useProjectIOStore } from '@/features/application/project/projectIOStore';
import { createNodeCatalogPublication } from '@/features/core/nodeCatalog/publication';
import { captureProjectLifecycleState } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  createProjectEventIngress,
  type ProjectEventIngress,
} from '@/features/application/project/projectEventIngress';
import {
  createProjectEventReconciler,
  type ProjectEventReconciler,
  type ProjectEventReconcilerDependencies,
} from '@/features/application/project/projectEventReconciler';
import {
  createProjectEventStream,
  type ProjectEventStream,
} from '@/services/project/projectEventStream';
import { resetGraphProjectionCoordinator, invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { applyProjectLifecycleReceipt } from '@/features/application/projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceiptDependencies';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { reconcileProjectComputationSettingsEvent } from '@/features/application/projectSettings/useProjectComputationSettings';

interface ProjectSyncRuntime {
  readonly stream: ProjectEventStream;
  readonly ingress: ProjectEventIngress;
  readonly unsubscribe: () => void;
  references: number;
  start: Promise<void> | null;
}

let runtime: ProjectSyncRuntime | null = null;

function hydrationDependencies(): ProjectEventReconcilerDependencies['hydration'] {
  return {
    loadCurrentProject: async () => (await useProjectIOStore.getState().loadProject())
      ? { status: 'published' as const }
      : { status: 'failed' as const },
    refreshResourceIndex: async () => (await useProjectIOStore.getState().refreshResourceIndex())
      ? { status: 'published' as const }
      : { status: 'failed' as const },
    loadGraph: async (graphPath) => (await useProjectIOStore.getState().loadGraph(graphPath))
      ? { status: 'published' as const }
      : { status: 'failed' as const },
    replaceProject: resetGraphProjectionCoordinator,
  };
}

function createReconciler(): ProjectEventReconciler {
  const hydration = hydrationDependencies();
  const publication = createNodeCatalogPublication();
  return createProjectEventReconciler({
    hydration,
    activateProject: async (result) => Boolean(await loadActivatedProject(result)),
    currentProjectInstanceId: () => captureProjectLifecycleState().projectInstanceId,
    publishProjectCleared: () => {
      projectPublicationCoordinator.cancelProject();
      const owner = captureProjectLifecycleState();
      return createProjectLifecycleReceiptDependencies().clearProject(owner);
    },
    publishLifecycleCommitted: async (result) => {
      await applyProjectLifecycleReceipt(
        result,
        'event',
        createProjectLifecycleReceiptDependencies(),
      );
    },
    publishProjectSaved: () => undefined,
    publishComputationSettingsChanged: (result) => {
      reconcileProjectComputationSettingsEvent(result);
    },
    publishGraphDelta: (payload) => {
      void invalidateGraphProjection(payload.delta.graphPath);
    },
    publishResourceMutationCommitted: async (result) => {
      await projectPublicationCoordinator.submit({ result: result as never });
      publication.observeResourcePublication(
        result.projectInstanceId,
        result.publicationRevision,
      );
    },
    requestAuthoritativeSnapshot: async () => {
      await useProjectIOStore.getState().loadProject();
    },
  });
}

function createRuntime(): ProjectSyncRuntime {
  const stream = createProjectEventStream();
  const ingress = createProjectEventIngress(createReconciler(), {
    requestAuthoritativeSnapshot: async () => {
      await useProjectIOStore.getState().loadProject();
    },
    publishIssue: (issue) => {
      if (issue.reason === 'recoveryRequested') return;
      useProjectIOStore.setState({
        error: { code: issue.code, incidentId: issue.incidentId },
      });
    },
  });
  const unsubscribe = stream.subscribe((item) => {
    ingress.enqueue(item);
  });
  return {
    stream,
    ingress,
    unsubscribe,
    references: 0,
    start: null,
  };
}

async function acquireRuntime(): Promise<ProjectSyncRuntime> {
  const current = runtime ?? createRuntime();
  runtime = current;
  current.references += 1;
  if (!current.start) {
    current.start = current.stream.start().then((outcome) => {
      if (!outcome.ok) current.ingress.enqueue({ kind: 'failure', issue: outcome.issue });
    });
  }
  await current.start;
  return current;
}

function releaseRuntime(current: ProjectSyncRuntime): void {
  if (runtime !== current) return;
  current.references = Math.max(current.references - 1, 0);
  if (current.references !== 0) return;
  runtime = null;
  current.unsubscribe();
  void current.ingress.closeAndDrain().then(() => current.stream.close());
}

/** Starts the one Application-owned Project event entrance for mounted windows. */
export function useProjectSync(): void {
  useEffect(() => {
    let current: ProjectSyncRuntime | null = null;
    let cancelled = false;
    void acquireRuntime().then((acquired) => {
      if (cancelled) {
        releaseRuntime(acquired);
        return;
      }
      current = acquired;
    });
    return () => {
      cancelled = true;
      if (current) releaseRuntime(current);
    };
  }, []);
}
