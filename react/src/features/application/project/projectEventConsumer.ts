import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { LifecycleMutationResultDto } from "@/shared/types/domain/project";
import type { ProjectEvent, ProjectLoadedPayload } from "@/services/project/projectEventParser";

type Awaitable<T> = T | PromiseLike<T>;

export type ProjectEventConsumptionOutcome =
  | { readonly status: "applied" }
  | { readonly status: "ignored" }
  | { readonly status: "recoveryRequested" };

export interface ProjectEventConsumerDependencies {
  readonly refreshResourceIndex: () => Awaitable<boolean>;
  readonly activateProject: (result: ProjectLoadedPayload["result"]) => Awaitable<boolean>;
  readonly currentProjectInstanceId: () => string | null;
  readonly publishProjectCleared?: (projectInstanceId: string) => Awaitable<void>;
  readonly publishLifecycleCommitted?: (result: LifecycleMutationResultDto) => Awaitable<void>;
  readonly publishResourceMutationCommitted?: (
    result: ResourceMutationResultDto,
  ) => Awaitable<void>;
}

export interface ProjectEventConsumer {
  acceptEvent(event: ProjectEvent): Promise<ProjectEventConsumptionOutcome>;
}

export function createProjectEventConsumer(
  dependencies: ProjectEventConsumerDependencies,
): ProjectEventConsumer {
  const acceptEvent = async (event: ProjectEvent): Promise<ProjectEventConsumptionOutcome> => {
    try {
      switch (event.type) {
        case "ProjectLoaded":
          return (await dependencies.activateProject(event.payload.result))
            ? { status: "applied" }
            : { status: "ignored" };
        case "ProjectCleared":
          await dependencies.publishProjectCleared?.(event.payload.projectInstanceId);
          return { status: "applied" };
        case "ProjectLifecycleCommitted":
          await dependencies.publishLifecycleCommitted?.(event.payload.result);
          return { status: "applied" };
        case "ProjectIndexInvalidated":
          if (dependencies.currentProjectInstanceId() !== event.payload.projectInstanceId) {
            return { status: "ignored" };
          }
          return (await dependencies.refreshResourceIndex())
            ? { status: "applied" }
            : { status: "recoveryRequested" };
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
