import { useEffect, useRef, useState } from 'react';
import { SourceService } from '@/services/resultSource/resultSourceService';
import type { SourceValue } from './types';

export function useSourceValue(sourceId: string | null) {
  const [value, setValue] = useState<SourceValue | null>(null);
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

    SourceService.getValue(sourceId)
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
