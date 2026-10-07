import { useRef, useState } from "react";

/** Keep the form mounted while a step runs, including a failed connection or import. */
export function useImportStep(action: (value: string) => Promise<string | null>) {
  const running = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (value: string) => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      setError(await action(value));
    } finally {
      running.current = false;
      setBusy(false);
    }
  };
  return { busy, error, setError, run };
}
