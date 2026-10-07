import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { InitializationState } from "./appInitialization.type";
import { LoadStatus } from "@/shared/types/ui";
import { initializeProjectForCurrentWindow } from "@/features/application/project";
import { logger } from "@/utils/frontendLogger";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { formatApplicationIpcError } from "@/features/application/errorReference";

export function useAppInitialization(): InitializationState {
  const { t } = useTranslation();
  const [state, setState] = useState<InitializationState>({
    status: LoadStatus.Idle,
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    setState({ status: LoadStatus.Loading, error: null });

    const syncProject = async () => {
      try {
        await initializeProjectForCurrentWindow();
        if (cancelled) return;
        setState({ status: LoadStatus.Ready, error: null });
      } catch (error) {
        if (cancelled) return;
        logger.sys.error("Failed to sync project: " + formatApplicationIpcError(error), "AppInit");
        setState({
          status: LoadStatus.Error,
          error: formatInlineUserError(error, t),
        });
      }
    };

    void syncProject();

    return () => {
      cancelled = true;
    };
  }, [t]);

  return state;
}
