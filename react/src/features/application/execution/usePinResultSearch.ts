import { useMemo } from "react";
import {
  filterPinResultSearchEntries,
  type PinResultSearchEntry,
} from "@/features/application/results/pinResultSearch";
import { usePinResultSearchEntries } from "@/features/application/results/runtime";

export function usePinResultSearch(graphPath: string, query: string) {
  const entries = usePinResultSearchEntries(graphPath);

  const filteredEntries = useMemo(
    () => filterPinResultSearchEntries(entries, query),
    [entries, query],
  );

  return {
    hasResults: entries.length > 0,
    entries: filteredEntries,
  };
}

export type { PinResultSearchEntry };
