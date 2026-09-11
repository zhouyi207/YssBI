import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import {
  PROJECT_ACTIVITY_PANEL_IDS,
  type ProjectActivityPanelId,
  type ActivityPanelSnapshot,
} from "@/shared/types/domain/activityPanel";
import type { ProjectIndexSnapshot } from "@/shared/types/domain/project";
import {
  useSidebarStore,
  type ActivityPanelPublication,
} from "@/features/core/sidebar/sidebarStore";
import { toErrorReference } from "@/features/application/errorReference";
import type { GraphEditorSessionDto } from "@/shared/types/domain/editorMutation";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { ChartDocument, ChartIndexEntry, ProjectIndexRow } from "@/shared/types";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { PreparedGraphProjectionReplacements } from "@/features/core/dataStore/graphProjectionStore";
import {
  acceptProjectLifecycleActivation,
  captureProjectIdentity,
  captureProjectLifecycleState,
  clearProjectLifecycle,
  isCurrentProjectIdentity,
  startProjectLifecycle,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { GraphMeta } from "@/features/core/dataStore/graphMetaStore";
import type { FocusedGraphSession } from "@/features/core/graphSession/graphSessionStore";
import type { EditorViewport } from "@/features/core/viewport/editorViewport";
import {
  useDocumentStateStore,
  useResourceStore,
  resourceKey,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";
import { isGraphDraftDirty, isGraphDraftSaving } from "@/features/core/graphDraft";
import { toProjectionEntities } from "@/features/domain/editorProjection";
import { ProjectService } from "@/services/project/projectService";
import { ChartService } from "@/services/chart/chartService";
import { clearChartPreviewCache } from "@/services/chart/chartPreviewCache";
import { prepareGraphSessionForPublication } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { clearChartLifecycleProjects } from "@/features/application/editor/chartLifecycleCoordinator";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { useNodeCatalogStore } from "@/features/core/nodeCatalog/nodeCatalogStore";
import { invalidateGraphResults } from "@/features/application/results/runtime";
import {
  collectResourceMutationGraphPaths,
  fingerprintResourceMutationResult,
} from "./resourceMutationResult";
import { validateResourceMutationResult } from "@/features/domain/resource/resourceMutationValidation";
import {
  buildProjectSnapshotPathRemaps,
  buildProjectSnapshotChartPathRemaps,
  commitPreparedProjectSnapshot,
  prepareProjectSnapshotCommit,
  validateProjectSnapshotIndex,
} from "./projectPublicationSnapshot";

export type ProjectPublicationSuccess = {
  status: "applied" | "duplicate" | "recovered";
  affectedGraphPaths: ReadonlySet<string>;
};
export type ProjectPublicationErrorCode =
  | "stale_project_lifecycle"
  | "publication_protocol_error"
  | "publication_recovery_failed";
export class ProjectPublicationError extends Error {
  constructor(
    readonly code: ProjectPublicationErrorCode,
    message: string,
    options?: { cause?: unknown },
  ) {
    super(message);
    this.name = "ProjectPublicationError";
    if (options) Object.assign(this, { cause: options.cause });
  }
}
export interface ProjectPublicationSubmission {
  result: ResourceMutationResultDto;
  fallbackPaths?: readonly string[];
  validate?: (result: ResourceMutationResultDto) => string | undefined;
}
export interface ProjectSnapshotPreparation {
  readonly projectInstanceId: string;
  readonly epoch: number;
  readonly publicationRevision: number;
  readonly index: ProjectIndexRow;
  readonly activityPanels: readonly ActivityPanelPublication[];
  readonly graphSessions: ReadonlyMap<string, GraphEditorSessionDto>;
  readonly chartDocuments: ReadonlyMap<string, ChartDocument>;
  readonly pathRemaps: ReadonlyMap<string, string>;
  readonly chartPathRemaps: ReadonlyMap<string, string>;
}
export interface PreparedProjectSnapshotStoreState {
  readonly resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>;
  readonly graphOrder: string[];
  readonly documents: Readonly<Record<ResourceKey, DocumentState>>;
  readonly graphMeta: Readonly<Record<string, GraphMeta>>;
  readonly databases: Readonly<Record<string, DatabaseRecord>>;
  readonly databaseRevisions: Readonly<Record<string, number>>;
  readonly chartIndex: ChartIndexEntry[];
  readonly chartDocuments: Readonly<Record<string, ChartDocument>>;
  readonly focusedSession: FocusedGraphSession | null;
  readonly viewports: Readonly<Record<string, EditorViewport>>;
}
export interface PreparedProjectSnapshot extends ProjectSnapshotPreparation {
  readonly graphProjectionPlan: PreparedGraphProjectionReplacements;
  readonly storeState: PreparedProjectSnapshotStoreState;
}
export interface ProjectPublicationDependencies {
  loadProjectIndex(
    projectInstanceId: string,
    locale: string,
    previous: Partial<Record<ProjectActivityPanelId, ActivityPanelSnapshot>>,
  ): Promise<ProjectIndexSnapshot>;
  loadChartDocument(
    projectInstanceId: string,
    path: string,
    publicationRevision: number,
  ): Promise<ChartDocument>;
  prepareGraphSession(
    path: string,
    projectInstanceId: string,
    epoch: number,
  ): Promise<GraphEditorSessionDto | false>;
  captureLoadedGraphPaths(): ReadonlySet<string>;
  prepareSnapshot(plan: ProjectSnapshotPreparation): PreparedProjectSnapshot;
  commitSnapshot(plan: PreparedProjectSnapshot): void | Promise<void>;
  markProjectProjectionStale(): void;
}
interface PublicationWaiter {
  resolve(value: ProjectPublicationSuccess): void;
  reject(error: ProjectPublicationError): void;
}
interface PendingPublication {
  input: ProjectPublicationSubmission;
  fingerprint: string;
  affectedGraphPaths: ReadonlySet<string>;
  waiters: PublicationWaiter[];
}
interface IndexWaiter {
  resolve(): void;
  reject(error: ProjectPublicationError): void;
}

function protocolError(message: string): ProjectPublicationError {
  return new ProjectPublicationError("publication_protocol_error", message);
}
function staleLifecycleError(): ProjectPublicationError {
  return new ProjectPublicationError(
    "stale_project_lifecycle",
    "project lifecycle changed before publication settlement",
  );
}
function indexSignature(index: ProjectIndexRow): string {
  return JSON.stringify([index.graphs, index.charts, index.databases]);
}

/** The only installer for resource receipts and index invalidations. */
export class ProjectPublicationCoordinator {
  private appliedRevision = 0;
  private appliedFingerprint: string | undefined;
  private publishedIndexSignature: string | undefined;
  private phase: "idle" | "applying" | "recovering" = "idle";
  private readonly pending = new Map<number, PendingPublication>();
  private indexWaiters: IndexWaiter[] = [];
  private driverInFlight: Promise<void> | null = null;

  constructor(private readonly dependencies: ProjectPublicationDependencies) {}

  validateProjectStart(projectInstanceId: string, revision: number): void {
    if (!projectInstanceId || !Number.isSafeInteger(revision) || revision < 0)
      throw protocolError("project publication baseline is malformed");
  }
  startProject(projectInstanceId: string, revision: number, index?: ProjectIndexRow): void {
    this.validateProjectStart(projectInstanceId, revision);
    clearChartPreviewCache();
    clearChartLifecycleProjects();
    startProjectLifecycle(projectInstanceId);
    this.reset(revision, index);
    useNodeCatalogStore.getState().observeResourcePublication(projectInstanceId, revision);
  }
  acceptProjectActivation(projectInstanceId: string, revision: number): boolean {
    if (!projectInstanceId || !Number.isSafeInteger(revision) || revision <= 0)
      throw protocolError("project activation identity is malformed");
    const result = acceptProjectLifecycleActivation(projectInstanceId, revision);
    if (result === "stale") return false;
    if (result === "activated") {
      clearChartPreviewCache();
      clearChartLifecycleProjects();
      this.reset(0);
    }
    return true;
  }
  cancelProject(): void {
    clearChartPreviewCache();
    clearChartLifecycleProjects();
    clearProjectLifecycle();
    this.reset(0);
  }
  private reset(revision: number, index?: ProjectIndexRow): void {
    const error = staleLifecycleError();
    for (const pending of this.pending.values())
      for (const waiter of pending.waiters) waiter.reject(error);
    for (const waiter of this.indexWaiters) waiter.reject(error);
    this.pending.clear();
    this.indexWaiters = [];
    this.appliedRevision = revision;
    this.appliedFingerprint = undefined;
    this.publishedIndexSignature = index ? indexSignature(index) : undefined;
    this.phase = "idle";
    this.driverInFlight = null;
    useNodeCatalogStore.getState().clear();
    useSidebarStore.getState().clearProjectPanels();
  }
  capturePublicationRevision(): number {
    return this.appliedRevision;
  }
  captureCommandLifecycle() {
    return { ...captureProjectIdentity(), publicationRevision: this.appliedRevision };
  }
  markProjectProjectionStale(): void {
    this.publishedIndexSignature = undefined;
    this.dependencies.markProjectProjectionStale();
  }
  getSnapshotForTests() {
    return {
      ...captureProjectLifecycleState(),
      appliedRevision: this.appliedRevision,
      phase: this.phase,
      pendingRevisions: [...this.pending.keys()].sort((a, b) => a - b),
    };
  }

  submit(input: ProjectPublicationSubmission): Promise<ProjectPublicationSuccess> {
    const identity = captureProjectLifecycleState();
    if (
      !identity.projectInstanceId ||
      input.result.projectInstanceId !== identity.projectInstanceId
    )
      return Promise.reject(staleLifecycleError());
    const invalid = validateResourceMutationResult(input.result) ?? input.validate?.(input.result);
    if (invalid) return Promise.reject(protocolError(invalid));
    const revision = input.result.publicationRevision;
    const fingerprint = fingerprintResourceMutationResult(input.result);
    const affectedGraphPaths = collectResourceMutationGraphPaths(
      input.result,
      input.fallbackPaths ?? [],
    );
    const existing = this.pending.get(revision);
    if (existing && existing.fingerprint !== fingerprint)
      return Promise.reject(protocolError("conflicting receipts for one publication revision"));
    if (!existing && revision <= this.appliedRevision) {
      if (
        revision === this.appliedRevision &&
        this.appliedFingerprint &&
        fingerprint !== this.appliedFingerprint
      )
        return Promise.reject(protocolError("conflicting receipt for the installed publication"));
      // An installed authoritative snapshot covers late command/event copies as well.
      return Promise.resolve({ status: "duplicate", affectedGraphPaths });
    }
    const pending = existing ?? { input, fingerprint, affectedGraphPaths, waiters: [] };
    this.pending.set(revision, pending);
    const promise = new Promise<ProjectPublicationSuccess>((resolve, reject) =>
      pending.waiters.push({ resolve, reject }),
    );
    this.kick();
    return promise;
  }

  refreshIndex(): Promise<void> {
    if (!captureProjectLifecycleState().projectInstanceId)
      return Promise.reject(staleLifecycleError());
    const promise = new Promise<void>((resolve, reject) =>
      this.indexWaiters.push({ resolve, reject }),
    );
    this.kick();
    return promise;
  }
  private assertCurrent(identity: ProjectIdentitySnapshot): void {
    if (!isCurrentProjectIdentity(identity)) throw staleLifecycleError();
  }
  private kick(): void {
    if (this.driverInFlight) return;
    const identity = captureProjectIdentity();
    const driver = Promise.resolve().then(() => this.drive(identity));
    this.driverInFlight = driver;
    void driver
      .finally(() => {
        if (this.driverInFlight !== driver) return;
        this.driverInFlight = null;
        this.phase = "idle";
        if (this.pending.size || this.indexWaiters.length) this.kick();
      })
      .catch(() => undefined);
  }
  private async drive(identity: ProjectIdentitySnapshot): Promise<void> {
    while (isCurrentProjectIdentity(identity) && (this.pending.size || this.indexWaiters.length)) {
      const first = this.pending.get(this.appliedRevision + 1);
      const receipt = first?.input.result;
      if (
        first &&
        receipt &&
        receipt.deltas.length === 0 &&
        receipt.moves.length === 0 &&
        receipt.projectionStatus.status === "complete" &&
        receipt.projectionReplacements.length === 0
      ) {
        this.appliedRevision = receipt.publicationRevision;
        this.appliedFingerprint = first.fingerprint;
        this.pending.delete(this.appliedRevision);
        useNodeCatalogStore
          .getState()
          .observeResourcePublication(identity.projectInstanceId, this.appliedRevision);
        for (const waiter of first.waiters)
          waiter.resolve({ status: "applied", affectedGraphPaths: first.affectedGraphPaths });
        continue;
      }
      const waiting = [...this.indexWaiters];
      const owned = new Set(this.pending.values());
      const recovered =
        (!first && owned.size > 0) ||
        [...owned].some((p) => p.input.result.projectionStatus.status === "incomplete");
      this.phase = recovered ? "recovering" : "applying";
      try {
        await this.publishIndex(identity, recovered);
        this.assertCurrent(identity);
        for (const waiter of waiting) waiter.resolve();
      } catch (cause) {
        const error = isCurrentProjectIdentity(identity)
          ? new ProjectPublicationError(
              "publication_recovery_failed",
              "authoritative project publication failed",
              { cause },
            )
          : staleLifecycleError();
        for (const [revision, pending] of this.pending) {
          if (!owned.has(pending)) continue;
          this.pending.delete(revision);
          for (const waiter of pending.waiters) waiter.reject(error);
        }
        for (const waiter of waiting) waiter.reject(error);
        if (isCurrentProjectIdentity(identity)) {
          this.markProjectProjectionStale();
        }
      }
      if (isCurrentProjectIdentity(identity))
        this.indexWaiters = this.indexWaiters.filter((waiter) => !waiting.includes(waiter));
    }
  }

  private async publishIndex(identity: ProjectIdentitySnapshot, recovered: boolean): Promise<void> {
    for (let attempt = 0; attempt < 2; attempt++) {
      const locale = currentProjectionLocale();
      const bindings = PROJECT_ACTIVITY_PANEL_IDS.map((panelId) =>
        useSidebarStore.getState().bindPanel({ panelId, ...identity, locale }),
      );
      try {
        const previous = Object.fromEntries(
          bindings.flatMap((binding) => {
            const snapshot = useSidebarStore.getState().panels[binding.panelId]?.snapshot;
            return snapshot ? [[binding.panelId, snapshot]] : [];
          }),
        );
        for (const binding of bindings) useSidebarStore.getState().startPanelRequest(binding);
        const response = await this.dependencies.loadProjectIndex(
          identity.projectInstanceId,
          locale,
          previous,
        );
        const index = response.index;
        const activityPanels = bindings.map((binding) => ({
          binding,
          snapshot: response.activityPanels[binding.panelId as ProjectActivityPanelId],
        }));
        this.assertCurrent(identity);
        const invalid = validateProjectSnapshotIndex(index, identity.projectInstanceId);
        if (invalid) throw protocolError(invalid);
        const minimum = Math.max(this.appliedRevision, useResourceStore.getState().indexRevision);
        if (
          index.publicationRevision < minimum ||
          (this.pending.size > 0 &&
            ![...this.pending.keys()].some((revision) => revision <= index.publicationRevision))
        )
          throw protocolError("index does not cover the requested publication");
        const graphSessions = new Map<string, GraphEditorSessionDto>();
        const chartDocuments = new Map<string, ChartDocument>();
        while (true) {
          const covered = [...this.pending.values()].filter(
            (p) => p.input.result.publicationRevision <= index.publicationRevision,
          );
          const receipts = covered.map((p) => p.input.result);
          const signature = indexSignature(index);
          const changed = signature !== this.publishedIndexSignature;
          if (
            changed ||
            receipts.some((r) => r.projectionReplacements.length > 0 || r.moves.length > 0)
          ) {
            const graphPaths = new Set(index.graphs.map((g) => g.path));
            const chartPaths = new Set(index.charts.map((c) => c.chartPath));
            const pathRemaps = buildProjectSnapshotPathRemaps(graphPaths, receipts);
            const chartPathRemaps = buildProjectSnapshotChartPathRemaps(chartPaths, receipts);
            const affected = new Set(covered.flatMap((p) => [...p.affectedGraphPaths]));
            const loaded = this.dependencies.captureLoadedGraphPaths();
            for (const graph of index.graphs) {
              const previousPath =
                [...pathRemaps].find(([, to]) => to === graph.path)?.[0] ?? graph.path;
              if (
                !loaded.has(previousPath) ||
                isGraphDraftDirty(previousPath) ||
                isGraphDraftSaving(previousPath) ||
                graphSessions.has(graph.path)
              )
                continue;
              const previous =
                useResourceStore.getState().resources[
                  resourceKey({ id: previousPath, kind: graph.type })
                ];
              if (
                !recovered &&
                previousPath === graph.path &&
                previous?.revision === graph.revision &&
                !affected.has(graph.path) &&
                !previous.hasStaleDocument
              )
                continue;
              const session = await this.dependencies.prepareGraphSession(
                graph.path,
                identity.projectInstanceId,
                identity.epoch,
              );
              this.assertCurrent(identity);
              if (!session || toProjectionEntities(session.projection).graphPath !== graph.path)
                throw protocolError("graph projection identity is invalid");
              graphSessions.set(graph.path, session);
            }
            for (const chart of index.charts) {
              const previousPath =
                [...chartPathRemaps].find(([, to]) => to === chart.chartPath)?.[0] ??
                chart.chartPath;
              const cached = useChartDocumentStore.getState().documents[previousPath];
              const previous =
                useResourceStore.getState().resources[
                  resourceKey({ id: previousPath, kind: "chart" })
                ];
              const dirty =
                useDocumentStateStore.getState().documents[
                  resourceKey({ id: previousPath, kind: "chart" })
                ]?.dirty;
              const created = receipts.some((r) =>
                r.deltas.some(
                  (d) =>
                    d.payload.kind === "resource_lifecycle" &&
                    d.payload.patch.after?.kind === "chart" &&
                    d.payload.patch.after.path === chart.chartPath,
                ),
              );
              if (
                dirty ||
                chartDocuments.has(chart.chartPath) ||
                (!created &&
                  (!cached ||
                    (previous?.revision === chart.revision &&
                      !previous.hasStaleDocument &&
                      previousPath === chart.chartPath)))
              )
                continue;
              const document = await this.dependencies.loadChartDocument(
                identity.projectInstanceId,
                chart.chartPath,
                index.publicationRevision,
              );
              this.assertCurrent(identity);
              chartDocuments.set(chart.chartPath, document);
            }
            // Include receipts delivered while documents were being prepared before committing moves.
            if (
              [...this.pending.values()].some(
                (p) =>
                  p.input.result.publicationRevision <= index.publicationRevision &&
                  !covered.includes(p),
              )
            )
              continue;
            const plan = this.dependencies.prepareSnapshot({
              ...identity,
              publicationRevision: index.publicationRevision,
              index,
              activityPanels,
              graphSessions,
              chartDocuments,
              pathRemaps,
              chartPathRemaps,
            });
            this.assertCurrent(identity);
            await this.dependencies.commitSnapshot(plan);
            this.assertCurrent(identity);
            for (const graphPath of affected) invalidateGraphResults(graphPath);
            this.publishedIndexSignature = signature;
          } else {
            useSidebarStore.getState().publishPanels(activityPanels);
          }
          const indexOnlyChange = changed && index.publicationRevision === this.appliedRevision;
          this.appliedRevision = index.publicationRevision;
          this.appliedFingerprint = covered.find(
            (p) => p.input.result.publicationRevision === index.publicationRevision,
          )?.fingerprint;
          useNodeCatalogStore
            .getState()
            .observeResourcePublication(
              identity.projectInstanceId,
              index.publicationRevision,
              indexOnlyChange,
            );
          for (const pending of covered) {
            const revision = pending.input.result.publicationRevision;
            this.pending.delete(revision);
            for (const waiter of pending.waiters)
              waiter.resolve({
                status: recovered ? "recovered" : "applied",
                affectedGraphPaths: pending.affectedGraphPaths,
              });
          }
          return;
        }
      } catch (error) {
        this.assertCurrent(identity);
        if (
          attempt === 1 ||
          (error as { code?: string })?.code === "activity_panel_contract_invalid"
        ) {
          for (const binding of bindings)
            useSidebarStore
              .getState()
              .failPanelRequest(binding, toErrorReference(error, "activity_panel_sync_failed"));
          throw error;
        }
      }
    }
  }
}
export const projectPublicationCoordinator = new ProjectPublicationCoordinator({
  loadProjectIndex: (id, locale, previous) => ProjectService.getProjectIndex(id, locale, previous),
  loadChartDocument: (id, path, revision) => ChartService.loadChart(id, path, revision),
  prepareGraphSession: prepareGraphSessionForPublication,
  captureLoadedGraphPaths: () =>
    new Set(Object.keys(useGraphProjectionStore.getState().graphEntities)),
  prepareSnapshot: prepareProjectSnapshotCommit,
  commitSnapshot: commitPreparedProjectSnapshot,
  markProjectProjectionStale: () => {
    useDocumentStateStore.setState((state) => ({
      documents: Object.fromEntries(
        Object.entries(state.documents).map(([key, document]) => [
          key,
          { ...document, stale: true },
        ]),
      ),
    }));
    useResourceStore.setState((state) => ({
      resources: Object.fromEntries(
        Object.entries(state.resources).map(([key, resource]) => [
          key,
          { ...resource, hasStaleDocument: true },
        ]),
      ),
    }));
  },
});
