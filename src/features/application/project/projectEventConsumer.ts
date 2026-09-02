import type {
  ResourceKeyDto,
  ResourceMutationResultDto,
} from "@/shared/types/domain/editorMutation";
import type { LifecycleMutationResultDto } from "@/shared/types/domain/project";
import type {
  ProjectHydrationCoordinator,
  ProjectHydrationOutcome,
} from "./projectHydrationCoordinator";

type Awaitable<T> = T | PromiseLike<T>;

export interface ProjectSaveReceipt {
  readonly projectInstanceId: string;
  readonly operationId: string;
  readonly publicationRevision: number;
  readonly affectedResources: readonly ResourceKeyDto[];
  readonly indexInvalidated: boolean;
  readonly history: {
    readonly canUndo: boolean;
    readonly canRedo: boolean;
  };
}

export interface ProjectLoadedPayload {
  readonly result: {
    readonly path: string;
    readonly projectInstanceId: string;
    readonly activationRevision: number;
  };
}

export interface ProjectLifecycleCommittedPayload {
  readonly result: LifecycleMutationResultDto;
}

export interface ProjectSavedPayload {
  readonly result: ProjectSaveReceipt;
}

export interface ProjectIndexInvalidatedPayload {
  readonly projectInstanceId: string;
  readonly source: "watcher";
  readonly version: number;
}

export interface ResourceMutationCommittedPayload {
  readonly result: ResourceMutationResultDto;
}

/** Low-rate Rust facts. Resource commands publish through their response only. */
export type ProjectEvent =
  | { readonly type: "ProjectLoaded"; readonly payload: ProjectLoadedPayload }
  | { readonly type: "ProjectCleared"; readonly payload: undefined }
  | {
      readonly type: "ProjectLifecycleCommitted";
      readonly payload: ProjectLifecycleCommittedPayload;
    }
  | { readonly type: "ProjectSaved"; readonly payload: ProjectSavedPayload }
  | {
      readonly type: "ProjectIndexInvalidated";
      readonly payload: ProjectIndexInvalidatedPayload;
    }
  | {
      readonly type: "ResourceMutationCommitted";
      readonly payload: ResourceMutationCommittedPayload;
    };

export type ProjectEventConsumptionOutcome =
  | { readonly status: "applied" }
  | { readonly status: "ignored" }
  | { readonly status: "recoveryRequested" };

export interface ProjectEventConsumerDependencies {
  readonly hydration: Pick<
    ProjectHydrationCoordinator,
    "loadCurrentProject" | "refreshResourceIndex" | "replaceProject"
  >;
  readonly activateProject?: (result: ProjectLoadedPayload["result"]) => Awaitable<boolean>;
  readonly currentProjectInstanceId: () => string | null;
  readonly publishProjectCleared?: () => Awaitable<void>;
  readonly publishLifecycleCommitted?: (result: LifecycleMutationResultDto) => Awaitable<void>;
  readonly publishProjectSaved?: (result: ProjectSaveReceipt) => Awaitable<void>;
  readonly publishResourceMutationCommitted?: (
    result: ResourceMutationResultDto,
  ) => Awaitable<void>;
}

export interface ProjectEventConsumer {
  acceptEvent(event: ProjectEvent): Promise<ProjectEventConsumptionOutcome>;
}

function hydrationOutcomeToConsumption(
  outcome: ProjectHydrationOutcome,
): ProjectEventConsumptionOutcome {
  if (outcome.status === "published") return { status: "applied" };
  if (outcome.status === "failed") return { status: "recoveryRequested" };
  return { status: "ignored" };
}

export function createProjectEventConsumer(
  dependencies: ProjectEventConsumerDependencies,
): ProjectEventConsumer {
  const acceptEvent = async (event: ProjectEvent): Promise<ProjectEventConsumptionOutcome> => {
    try {
      switch (event.type) {
        case "ProjectLoaded": {
          if (dependencies.activateProject) {
            return (await dependencies.activateProject(event.payload.result))
              ? { status: "applied" }
              : { status: "ignored" };
          }
          dependencies.hydration.replaceProject();
          return hydrationOutcomeToConsumption(await dependencies.hydration.loadCurrentProject());
        }
        case "ProjectCleared":
          dependencies.hydration.replaceProject();
          await dependencies.publishProjectCleared?.();
          return { status: "applied" };
        case "ProjectLifecycleCommitted":
          await dependencies.publishLifecycleCommitted?.(event.payload.result);
          return { status: "applied" };
        case "ProjectSaved":
          if (dependencies.currentProjectInstanceId() !== event.payload.result.projectInstanceId) {
            return { status: "ignored" };
          }
          await dependencies.publishProjectSaved?.(event.payload.result);
          return { status: "applied" };
        case "ProjectIndexInvalidated":
          if (dependencies.currentProjectInstanceId() !== event.payload.projectInstanceId) {
            return { status: "ignored" };
          }
          return hydrationOutcomeToConsumption(await dependencies.hydration.refreshResourceIndex());
        case "ResourceMutationCommitted":
          if (dependencies.currentProjectInstanceId() !== event.payload.result.projectInstanceId) {
            return { status: "ignored" };
          }
          await dependencies.publishResourceMutationCommitted?.(event.payload.result);
          return { status: "applied" };
      }
    } catch {
      return { status: "recoveryRequested" };
    }
  };

  return { acceptEvent };
}
