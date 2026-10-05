import { useEffect, useMemo, useRef, useState } from "react";
import { useResourceRead } from "@/features/core/resource/read";
import {
  captureProjectReadContext,
  useProjectIOStore,
} from "@/features/application/project/projectIOStore";
import { KnowledgeService, type KnowledgeSource } from "@/services/assistant/knowledgeService";

// Panel-local query state. Source selection and freshness remain Rust-owned.
export function useProjectKnowledge() {
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const resources = useResourceRead((state) => state.resources);
  const documents = useMemo(
    () =>
      Object.values(resources)
        .filter((resource) => resource.kind === "doc" && resource.exists)
        .sort((left, right) => left.name.localeCompare(right.name)),
    [resources],
  );
  const documentBasis = documents.map(({ id, revision }) => `${id}:${revision}`).join("\n");
  const [sources, setSources] = useState<readonly KnowledgeSource[]>([]);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<string | null>(null);
  const [failure, setFailure] = useState<"load" | "mutation" | null>(null);
  const [refresh, setRefresh] = useState(0);
  const generation = useRef(0);
  const mutating = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    const current = ++generation.current;
    setSources([]);
    setFailure(null);
    const context = captureProjectReadContext(projectInstanceId);
    if (!context) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void KnowledgeService.list(context.projectInstanceId)
      .then((sources) => {
        if (current === generation.current && context.isCurrent()) setSources(sources);
      })
      .catch(() => {
        if (current === generation.current && context.isCurrent()) setFailure("load");
      })
      .finally(() => {
        if (current === generation.current) setLoading(false);
      });
    return () => {
      ++generation.current;
    };
  }, [projectInstanceId, documentBasis, refresh]);

  async function mutate(key: string, action: (project: string) => Promise<void>) {
    const context = captureProjectReadContext(projectInstanceId);
    if (!context || mutating.current) return;
    mutating.current = true;
    setPending(key);
    setFailure(null);
    try {
      await action(context.projectInstanceId);
      if (mounted.current && context.isCurrent()) setRefresh((value) => value + 1);
    } catch {
      if (mounted.current && context.isCurrent()) setFailure("mutation");
    } finally {
      mutating.current = false;
      if (mounted.current) setPending(null);
    }
  }

  return {
    projectInstanceId,
    documents,
    sources,
    loading,
    pending,
    failure,
    reload: () => setRefresh((value) => value + 1),
    rebuild: (path: string) => mutate(path, (project) => KnowledgeService.rebuild(project, path)),
    remove: (sourceId: string) =>
      mutate(sourceId, (project) => KnowledgeService.remove(project, sourceId)),
  };
}
