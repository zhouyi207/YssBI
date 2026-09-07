import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { ApplicationSettingsService } from "@/services/settings/applicationSettingsService";
import {
  RECOMMENDED_COMPUTATION_SETTINGS,
  type ApplicationSettingsSnapshotDto,
  type ComputationSettingsDto,
  type StatisticalMissingValuePolicy,
} from "@/shared/types/dto/applicationSettings";

export interface ApplicationComputationSettingsDraft {
  statistics: StatisticalMissingValuePolicy;
}

function toDraft(settings: ComputationSettingsDto): ApplicationComputationSettingsDraft {
  return {
    statistics: settings.missingValues.statistics,
  };
}

function draftSettings(draft: ApplicationComputationSettingsDraft): ComputationSettingsDto {
  return {
    missingValues: { statistics: draft.statistics },
  };
}

function settingsEqual(left: ComputationSettingsDto, right: ComputationSettingsDto): boolean {
  return left.missingValues.statistics === right.missingValues.statistics;
}

export function useApplicationComputationSettings() {
  const { t } = useTranslation();
  const [confirmed, setConfirmed] = useState<ApplicationSettingsSnapshotDto | null>(null);
  const [draft, replaceDraft] = useState<ApplicationComputationSettingsDraft>(() =>
    toDraft(RECOMMENDED_COMPUTATION_SETTINGS),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isApplying, setIsApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    setError(null);
    void ApplicationSettingsService.get()
      .then((snapshot) => {
        if (disposed) return;
        setConfirmed(snapshot);
        replaceDraft(toDraft(snapshot.settings.computation));
      })
      .catch((loadError: unknown) => {
        if (!disposed) setError(formatInlineUserError(loadError, t));
      })
      .finally(() => {
        if (!disposed) setIsLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [t]);

  const parsedDraft = useMemo(() => draftSettings(draft), [draft]);
  const isDirty = Boolean(confirmed && !settingsEqual(parsedDraft, confirmed.settings.computation));

  const setDraft = useCallback((patch: Partial<ApplicationComputationSettingsDraft>) => {
    replaceDraft((current) => ({ ...current, ...patch }));
  }, []);

  const apply = useCallback(async () => {
    if (!confirmed || isLoading) return;
    const operationId = crypto.randomUUID();
    setIsApplying(true);
    setError(null);
    try {
      const receipt = await ApplicationSettingsService.update({
        operationId,
        expectedRevision: confirmed.settingsRevision,
        settings: { computation: parsedDraft },
      });
      if (receipt.operationId !== operationId) {
        throw new Error("Application settings receipt correlation is invalid.");
      }
      if (receipt.settingsRevision !== confirmed.settingsRevision + 1) {
        throw new Error("Application settings receipt revision is invalid.");
      }
      setConfirmed(receipt);
      replaceDraft(toDraft(receipt.settings.computation));
    } catch (applyError) {
      setError(formatInlineUserError(applyError, t));
      throw applyError;
    } finally {
      setIsApplying(false);
    }
  }, [confirmed, isLoading, parsedDraft, t]);

  const restoreRecommended = useCallback(() => {
    replaceDraft(toDraft(RECOMMENDED_COMPUTATION_SETTINGS));
  }, []);

  return {
    enabled: !isLoading,
    confirmed,
    draft,
    isLoading,
    isApplying,
    isDirty,
    error,
    setDraft,
    apply,
    restoreRecommended,
  };
}
