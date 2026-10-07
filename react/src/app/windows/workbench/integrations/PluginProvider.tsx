import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { useStore } from "zustand";
import {
  createPluginRegistry,
  type PluginRegistrySnapshot,
} from "@/features/application/plugins/pluginRegistry";

const PluginContext = createContext<ReturnType<typeof createPluginRegistry> | null>(null);

export function PluginProvider({ children }: { children: ReactNode }) {
  const [registry] = useState(createPluginRegistry);
  useEffect(() => {
    void registry.start();
    return registry.stop;
  }, [registry]);
  return <PluginContext.Provider value={registry}>{children}</PluginContext.Provider>;
}

function usePluginRegistry() {
  const registry = useContext(PluginContext);
  if (!registry) throw new Error("PluginProvider is required");
  return registry;
}

export function usePlugins<T>(selector: (state: PluginRegistrySnapshot) => T): T {
  return useStore(usePluginRegistry().read, selector);
}

export function usePluginActions() {
  return usePluginRegistry().actions;
}
