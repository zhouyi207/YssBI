import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { hydrateGraphProjections } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { useResourceStore } from "@/features/core/resource";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";

export function useProjectionLocaleSync(): void {
  const { i18n } = useTranslation();
  const language = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
  const previousLanguage = useRef(language);

  useEffect(() => {
    if (previousLanguage.current === language) return;
    previousLanguage.current = language;

    const loadedGraphPaths = Object.values(useResourceStore.getState().resources)
      .filter(
        (resource) =>
          resource.loaded && (resource.kind === "event" || resource.kind === "function"),
      )
      .map((resource) => resource.id);

    void hydrateGraphProjections(loadedGraphPaths, language);
  }, [language]);
}
