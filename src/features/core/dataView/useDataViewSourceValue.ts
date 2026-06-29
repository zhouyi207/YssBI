import { useEffect, useRef, useState } from 'react';
import { DataViewService } from './dataViewService';
import type { DataViewSourceValue } from './types';

export function useDataViewSourceValue(sourceId: string | null) {
  const [value, setValue] = useState<DataViewSourceValue | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    if (!sourceId) {
      setValue(null);
      setLoading(false);
      setError(null);
      return;
    }

    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);

    DataViewService.getValue(sourceId)
      .then((next) => {
        if (requestId !== requestRef.current) return;
        setValue(next);
      })
      .catch((e) => {
        if (requestId !== requestRef.current) return;
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (requestId === requestRef.current) setLoading(false);
      });
  }, [sourceId]);

  return { value, loading, error };
}
