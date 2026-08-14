import { useEffect, useRef, useState } from 'react';
import { ResultService } from '@/services/result/resultService';
import type { ResultValue } from '@/shared/types/dto/result';

export function useResultValue(resultId: string | null) {
  const [value, setValue] = useState<ResultValue | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    if (!resultId) {
      setValue(null);
      setLoading(false);
      setError(null);
      return;
    }
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    ResultService.getValue(resultId)
      .then((next) => { if (requestId === requestRef.current) setValue(next); })
      .catch((cause) => {
        if (requestId === requestRef.current) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => { if (requestId === requestRef.current) setLoading(false); });
  }, [resultId]);

  return { value, loading, error };
}
