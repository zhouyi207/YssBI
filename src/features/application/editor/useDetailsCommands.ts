import { useMemo } from "react";

import { useVariableManagement } from "@/features/application/dataManagement";

export function useDetailsCommands() {
  const { updateVariable } = useVariableManagement();
  return useMemo(() => ({ updateVariable }), [updateVariable]);
}
