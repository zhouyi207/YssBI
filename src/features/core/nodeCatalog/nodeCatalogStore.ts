import { create } from 'zustand';
import type {
  LocalizedCatalogCategory,
  LocalizedCatalogItem,
} from '@/features/domain/nodeCatalog/catalogItem';

export interface LocalizedCatalogResponse {
  projectInstanceId: string;
  registryFingerprint: string;
  resourcePublicationRevision: number;
  locale: string;
  categories: LocalizedCatalogCategory[];
  items: LocalizedCatalogItem[];
}

export type CatalogLoadStatus = 'idle' | 'loading' | 'ready' | 'error';

export interface CatalogRequestIdentity {
  projectInstanceId: string;
  locale: string;
  requestGeneration: number;
}

export interface CatalogRequestState {
  status: CatalogLoadStatus;
  responseKey: string | null;
  error: string | null;
  requestGeneration: number | null;
}

export interface NodeCatalogState {
  responses: Record<string, LocalizedCatalogResponse>;
  requests: Record<string, CatalogRequestState>;
  beginRequest(projectInstanceId: string, locale: string): CatalogRequestIdentity | null;
  storeResponse(identity: CatalogRequestIdentity, response: LocalizedCatalogResponse): boolean;
  storeError(identity: CatalogRequestIdentity, error: string): boolean;
  clear(): void;
}

const IDLE_REQUEST: CatalogRequestState = {
  status: 'idle',
  responseKey: null,
  error: null,
  requestGeneration: null,
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
  return request.responseKey ? state.responses[request.responseKey] ?? null : null;
}

function ownsRequest(state: NodeCatalogState, identity: CatalogRequestIdentity): boolean {
  const request = state.requests[catalogRequestKey(identity.projectInstanceId, identity.locale)];
  return request?.status === 'loading'
    && request.requestGeneration === identity.requestGeneration;
}

export const useNodeCatalogStore = create<NodeCatalogState>((set) => ({
  responses: {},
  requests: {},

  beginRequest: (projectInstanceId, locale) => {
    let identity: CatalogRequestIdentity | null = null;
    set((state) => {
      if (selectCatalogRequest(state, projectInstanceId, locale).status === 'loading') return state;
      identity = { projectInstanceId, locale, requestGeneration: nextRequestGeneration++ };
      return {
        requests: {
          ...state.requests,
          [catalogRequestKey(projectInstanceId, locale)]: {
            status: 'loading',
            responseKey: null,
            error: null,
            requestGeneration: identity.requestGeneration,
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
      if (response.projectInstanceId !== identity.projectInstanceId
        || response.locale !== identity.locale) return state;
      stored = true;
      const responseKey = catalogResponseKey(response);
      return {
        responses: { ...state.responses, [responseKey]: response },
        requests: {
          ...state.requests,
          [catalogRequestKey(identity.projectInstanceId, identity.locale)]: {
            status: 'ready',
            responseKey,
            error: null,
            requestGeneration: identity.requestGeneration,
          },
        },
      };
    });
    return stored;
  },

  storeError: (identity, error) => {
    let stored = false;
    set((state) => {
      if (!ownsRequest(state, identity)) return state;
      stored = true;
      return {
        requests: {
          ...state.requests,
          [catalogRequestKey(identity.projectInstanceId, identity.locale)]: {
            status: 'error',
            responseKey: null,
            error,
            requestGeneration: identity.requestGeneration,
          },
        },
      };
    });
    return stored;
  },

  clear: () => set({ responses: {}, requests: {} }),
}));
