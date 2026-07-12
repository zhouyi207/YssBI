import { useEffect, useState } from 'react';
import { GraphService } from '@/services/graph/graphService';
import type { FunctionCallSiteDTO } from '@/shared/types/dto';

export function useFunctionCallSites(functionPath: string | undefined): {
  sites: FunctionCallSiteDTO[];
  loading: boolean;
} {
  const [sites, setSites] = useState<FunctionCallSiteDTO[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!functionPath) {
      setSites([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    void GraphService.getFunctionCallSites(functionPath)
      .then((result) => {
        if (!cancelled) setSites(result);
      })
      .catch(() => {
        if (!cancelled) setSites([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [functionPath]);

  return { sites, loading };
}
