import { create } from "zustand";
import { produce } from "immer";
import type { ErrorReference } from "@/shared/types/domain/errorReference";
import type {
  LocalizedCatalogCategory,
  LocalizedCatalogItem,
} from "@/features/domain/nodeCatalog/catalogItem";

export interface LocalizedCatalogResponse {
  projectInstanceId: string;
  registryFingerprint: string;
  resourcePublicationRevision: number;
  locale: string;
  categories: LocalizedCatalogCategory[];
  items: LocalizedCatalogItem[];
}

export type CatalogLoadStatus = "idle" | "loading" | "ready" | "error";

export const CATALOG_RESPONSE_CONTRACT_ERROR_CODE = "catalog_response_contract_error";
export const CATALOG_RESPONSE_STALE_ERROR_CODE = "catalog_response_stale";

export interface CatalogRequestIdentity {
  projectInstanceId: string;
  locale: string;
  requestGeneration: number;
  minimumResourcePublicationRevision: number;
}

export interface CatalogRequestState {
  status: CatalogLoadStatus;
  responseKey: string | null;
  error: ErrorReference | null;
  requestGeneration: number | null;
  minimumResourcePublicationRevision: number;
}

export interface NodeCatalogState {
  responses: Record<string, LocalizedCatalogResponse>;
  requests: Record<string, CatalogRequestState>;
  projectWatermarks: Record<string, number>;
  beginRequest(projectInstanceId: string, locale: string): CatalogRequestIdentity | null;
  storeResponse(identity: CatalogRequestIdentity, response: LocalizedCatalogResponse): boolean;
  storeError(identity: CatalogRequestIdentity, error: ErrorReference): boolean;
  observeResourcePublication(
    projectInstanceId: string,
    revision: number,
    indexInvalidated?: boolean,
  ): boolean;
  requestRefresh(projectInstanceId: string, locale: string): void;
  clear(): void;
}

const IDLE_REQUEST: CatalogRequestState = {
  status: "idle",
  responseKey: null,
  error: null,
  requestGeneration: null,
  minimumResourcePublicationRevision: 0,
};

let nextRequestGeneration = 1;

function catalogRequestKey(projectInstanceId: string, locale: string): string {
  return JSON.stringify([projectInstanceId, locale]);
}

export function catalogResponseKey(response: LocalizedCatalogResponse): string {
  return JSON.stringify([
    response.projectInstanceId,
    response.locale,
    response.registryFingerprint,
    response.resourcePublicationRevision,
  ]);
}

export function selectCatalogRequest(
  state: NodeCatalogState,
  projectInstanceId: string,
  locale: string,
): CatalogRequestState {
  return state.requests[catalogRequestKey(projectInstanceId, locale)] ?? IDLE_REQUEST;
}

export function selectCatalogResponse(
  state: NodeCatalogState,
  projectInstanceId: string,
  locale: string,
): LocalizedCatalogResponse | null {
  const request = selectCatalogRequest(state, projectInstanceId, locale);
  return request.responseKey ? (state.responses[request.responseKey] ?? null) : null;
}

function ownsRequest(state: NodeCatalogState, identity: CatalogRequestIdentity): boolean {
  const request = state.requests[catalogRequestKey(identity.projectInstanceId, identity.locale)];
  return request?.status === "loading" && request.requestGeneration === identity.requestGeneration;
}

export const useNodeCatalogStore = create<NodeCatalogState>((set) => ({
  responses: {},
  requests: {},
  projectWatermarks: {},

  beginRequest: (projectInstanceId, locale) => {
    let identity: CatalogRequestIdentity | null = null;
    set((state) => {
      const current = selectCatalogRequest(state, projectInstanceId, locale);
      if (current.status === "loading") return state;
      identity = {
        projectInstanceId,
        locale,
        requestGeneration: nextRequestGeneration++,
        minimumResourcePublicationRevision: Math.max(
          current.minimumResourcePublicationRevision,
          state.projectWatermarks[projectInstanceId] ?? 0,
        ),
      };
      return {
        requests: {
          ...state.requests,
          [catalogRequestKey(projectInstanceId, locale)]: {
            status: "loading",
            responseKey: current.responseKey,
            error: null,
            requestGeneration: identity.requestGeneration,
            minimumResourcePublicationRevision: identity.minimumResourcePublicationRevision,
          },
        },
      };
    });
    return identity;
  },

  storeResponse: (identity, response) => {
    let stored = false;
    set((state) => {
      if (!ownsRequest(state, identity)) return state;
      const requestKey = catalogRequestKey(identity.projectInstanceId, identity.locale);
      const request = state.requests[requestKey];
      if (
        response.projectInstanceId !== identity.projectInstanceId ||
        response.locale !== identity.locale
      ) {
        return {
          requests: {
            ...state.requests,
            [requestKey]: {
              ...request,
              status: "error",
              error: {
                code: CATALOG_RESPONSE_CONTRACT_ERROR_CODE,
                incidentId: null,
              },
            },
          },
        };
      }
      if (response.resourcePublicationRevision < identity.minimumResourcePublicationRevision) {
        return {
          requests: {
            ...state.requests,
            [requestKey]: {
              ...request,
              status: "error",
              error: {
                code: CATALOG_RESPONSE_STALE_ERROR_CODE,
                incidentId: null,
              },
              requestGeneration: null,
            },
          },
        };
      }
      stored = true;
      const responseKey = catalogResponseKey(response);
      return produce(state, (draft) => {
        // A response key includes its project and locale, so only this request owns it.
        if (request.responseKey && request.responseKey !== responseKey)
          delete draft.responses[request.responseKey];
        draft.responses[responseKey] = response;
        draft.projectWatermarks[identity.projectInstanceId] = Math.max(
          state.projectWatermarks[identity.projectInstanceId] ?? 0,
          response.resourcePublicationRevision,
        );
        draft.requests[requestKey] = {
          status: "ready",
          responseKey,
          error: null,
          requestGeneration: identity.requestGeneration,
          minimumResourcePublicationRevision: identity.minimumResourcePublicationRevision,
        };
      });
    });
    return stored;
  },

  storeError: (identity, error) => {
    let stored = false;
    set((state) => {
      if (!ownsRequest(state, identity)) return state;
      stored = true;
      const requestKey = catalogRequestKey(identity.projectInstanceId, identity.locale);
      const current = state.requests[requestKey];
      return {
        requests: {
          ...state.requests,
          [requestKey]: {
            status: "error",
            responseKey: current.responseKey,
            error,
            requestGeneration: identity.requestGeneration,
            minimumResourcePublicationRevision: current.minimumResourcePublicationRevision,
          },
        },
      };
    });
    return stored;
  },

  observeResourcePublication: (projectInstanceId, revision, indexInvalidated = false) => {
    let advanced = false;
    set(
      produce((state: NodeCatalogState) => {
        const currentRevision = state.projectWatermarks[projectInstanceId] ?? 0;
        if (!Number.isSafeInteger(revision) || revision < 0) return;
        const minimumRevision = Math.max(revision, currentRevision);
        advanced = minimumRevision > currentRevision || indexInvalidated;
        for (const [key, request] of Object.entries(state.requests)) {
          const [requestProject] = JSON.parse(key) as [string, string];
          if (
            requestProject !== projectInstanceId ||
            (request.minimumResourcePublicationRevision >= minimumRevision && !indexInvalidated)
          )
            continue;
          const cached = request.responseKey ? state.responses[request.responseKey] : null;
          const fresh =
            !indexInvalidated &&
            request.status === "ready" &&
            cached &&
            cached.resourcePublicationRevision >= minimumRevision;
          request.minimumResourcePublicationRevision = minimumRevision;
          if (!fresh) {
            request.status = "idle";
            request.error = null;
            request.requestGeneration = null;
          }
          advanced = true;
        }
        if (advanced) state.projectWatermarks[projectInstanceId] = minimumRevision;
      }),
    );
    return advanced;
  },

  requestRefresh: (projectInstanceId, locale) =>
    set((state) => {
      const key = catalogRequestKey(projectInstanceId, locale);
      const current = selectCatalogRequest(state, projectInstanceId, locale);
      if (current.status === "loading" || current.status === "idle") return state;
      return {
        requests: {
          ...state.requests,
          [key]: { ...current, status: "idle", error: null, requestGeneration: null },
        },
      };
    }),

  clear: () => set({ responses: {}, requests: {}, projectWatermarks: {} }),
}));
