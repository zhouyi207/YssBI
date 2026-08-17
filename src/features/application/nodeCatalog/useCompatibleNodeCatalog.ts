import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { getLocalizedSearchIndex } from '@/features/core/nodeCatalog/localizedSearchIndex';
import {
  CATALOG_RESPONSE_CONTRACT_ERROR_CODE,
  type LocalizedCatalogResponse,
} from '@/features/core/nodeCatalog/nodeCatalogStore';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { CatalogService } from '@/services/nodeSystem/catalogService';
import { toErrorReference } from '@/services/ipc';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { DEFAULT_LANGUAGE } from '@/shared/types/settings';
import type { LocalizedNodeCatalogState } from './useLocalizedNodeCatalog';

interface CompatibleNodeCatalogInput {
  enabled: boolean;
  graphPath: string | null;
  graphRevision: number | null;
  sourcePort: PortAddressDto | null;
}

interface CompatibleRequestState {
  status: LocalizedNodeCatalogState['status'];
  error: LocalizedNodeCatalogState['error'];
  catalog: LocalizedCatalogResponse | null;
}

const IDLE_STATE: CompatibleRequestState = {
  status: 'idle',
  error: null,
  catalog: null,
};

export function useCompatibleNodeCatalog({
  enabled,
  graphPath,
  graphRevision,
  sourcePort,
}: CompatibleNodeCatalogInput): LocalizedNodeCatalogState {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [refreshGeneration, setRefreshGeneration] = useState(0);
  const [state, setState] = useState<CompatibleRequestState>(IDLE_STATE);
  const sourcePortKey = sourcePort ? JSON.stringify(sourcePort) : '';

  useEffect(() => {
    if (!enabled || !projectInstanceId || !graphPath || graphRevision === null || !sourcePort) {
      setState(IDLE_STATE);
      return;
    }

    let identity: ReturnType<typeof captureProjectIdentity>;
    try {
      identity = captureProjectIdentity();
    } catch {
      setState(IDLE_STATE);
      return;
    }
    if (identity.projectInstanceId !== projectInstanceId) return;

    let current = true;
    setState({ status: 'loading', error: null, catalog: null });
    void CatalogService.getCompatibleNodeCatalog({
      projectInstanceId,
      graphPath,
      graphRevision,
      sourcePort,
      locale,
    }).then((catalog) => {
      if (!current || !isCurrentProjectIdentity(identity)) return;
      if (useProjectIOStore.getState().projectInstanceId !== identity.projectInstanceId) return;
      if (catalog.projectInstanceId !== identity.projectInstanceId || catalog.locale !== locale) {
        setState({
          status: 'error',
          error: {
            code: CATALOG_RESPONSE_CONTRACT_ERROR_CODE,
            incidentId: null,
          },
          catalog: null,
        });
        return;
      }
      setState({ status: 'ready', error: null, catalog });
    }).catch((error: unknown) => {
      if (!current || !isCurrentProjectIdentity(identity)) return;
      if (useProjectIOStore.getState().projectInstanceId !== identity.projectInstanceId) return;
      setState({
        status: 'error',
        error: toErrorReference(error, CATALOG_RESPONSE_CONTRACT_ERROR_CODE),
        catalog: null,
      });
    });

    return () => {
      current = false;
    };
  }, [
    enabled,
    graphPath,
    graphRevision,
    locale,
    projectInstanceId,
    refreshGeneration,
    sourcePortKey,
  ]);

  const refresh = useCallback(() => {
    if (enabled) setRefreshGeneration((generation) => generation + 1);
  }, [enabled]);
  const searchIndex = useMemo(
    () => state.catalog ? getLocalizedSearchIndex(state.catalog) : null,
    [state.catalog],
  );

  return {
    status: state.status,
    error: state.error,
    catalog: state.catalog,
    searchIndex,
    refresh,
  };
}
