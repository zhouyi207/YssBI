import { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { DEFAULT_LANGUAGE } from '@/shared/types/settings';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import {
  getLocalizedSearchIndex,
  type LocalizedSearchIndex,
} from '@/features/core/nodeCatalog/localizedSearchIndex';
import {
  selectCatalogRequest,
  selectCatalogResponse,
  useNodeCatalogStore,
  type CatalogLoadStatus,
  type LocalizedCatalogResponse,
} from '@/features/core/nodeCatalog/nodeCatalogStore';
import { CatalogService } from '@/services/nodeSystem/catalogService';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';

export interface LocalizedNodeCatalogState {
  status: CatalogLoadStatus;
  error: string | null;
  catalog: LocalizedCatalogResponse | null;
  searchIndex: LocalizedSearchIndex | null;
  refresh(): void;
}

export function useLocalizedNodeCatalog(): LocalizedNodeCatalogState {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const request = useNodeCatalogStore((state) => projectInstanceId
    ? selectCatalogRequest(state, projectInstanceId, locale)
    : null);
  const catalog = useNodeCatalogStore((state) => projectInstanceId
    ? selectCatalogResponse(state, projectInstanceId, locale)
    : null);

  useEffect(() => {
    if (!projectInstanceId || request?.status === 'loading' || request?.status === 'ready'
      || request?.status === 'error') return;

    let identity: ReturnType<typeof captureProjectIdentity>;
    try {
      identity = captureProjectIdentity();
    } catch {
      return;
    }
    if (identity.projectInstanceId !== projectInstanceId) return;

    const requestIdentity = useNodeCatalogStore
      .getState()
      .beginRequest(projectInstanceId, locale);
    if (!requestIdentity) return;

    void CatalogService.getLocalizedCatalog(projectInstanceId, locale)
      .then((response) => {
        if (!isCurrentProjectIdentity(identity)) return;
        if (useProjectIOStore.getState().projectInstanceId !== identity.projectInstanceId) return;
        if (response.projectInstanceId !== identity.projectInstanceId || response.locale !== locale) return;
        useNodeCatalogStore.getState().storeResponse(requestIdentity, response);
      })
      .catch((error: unknown) => {
        if (!isCurrentProjectIdentity(identity)) return;
        if (useProjectIOStore.getState().projectInstanceId !== identity.projectInstanceId) return;
        useNodeCatalogStore.getState().storeError(
          requestIdentity,
          formatErrorMessage(error, 'Failed to load node catalog'),
        );
      });
  }, [locale, projectInstanceId, request?.status]);

  const refresh = useCallback(() => {
    if (!projectInstanceId) return;
    useNodeCatalogStore.getState().requestRefresh(projectInstanceId, locale);
  }, [locale, projectInstanceId]);

  return {
    status: request?.status ?? 'idle',
    error: request?.error ?? null,
    catalog,
    searchIndex: catalog ? getLocalizedSearchIndex(catalog) : null,
    refresh,
  };
}
