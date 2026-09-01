import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { ProjectService } from "@/services/project/projectService";
import { uiStore } from "@/features/core/ui/UIStore";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import {
  RECOMMENDED_PROJECT_COMPUTATION_SETTINGS,
  type ComputationSettingsMutationReceiptDto,
  type ComputationSettingsSnapshotDto,
  type ProjectComputationSettingsDto,
  type StatisticalMissingValuePolicy,
} from "@/shared/types/domain/projectComputationSettings";

export interface ProjectComputationSettingsDraft {
  absolute: string;
  relative: string;
  statistics: StatisticalMissingValuePolicy;
}

type AuthorityListener = (snapshot: ComputationSettingsSnapshotDto) => void;
const authorityListeners = new Set<AuthorityListener>();
const latestByProject = new Map<string, ComputationSettingsSnapshotDto>();

function publish(snapshot: ComputationSettingsSnapshotDto): boolean {
  const current = latestByProject.get(snapshot.projectInstanceId);
  if (current && current.settingsRevision >= snapshot.settingsRevision) return false;
  latestByProject.set(snapshot.projectInstanceId, snapshot);
  authorityListeners.forEach((listener) => listener(snapshot));
  return true;
}

export function reconcileProjectComputationSettingsEvent(
  receipt: ComputationSettingsMutationReceiptDto,
): boolean {
  return publish(receipt);
}

function toDraft(settings: ProjectComputationSettingsDto): ProjectComputationSettingsDraft {
  return {
    absolute: String(settings.numeric.tolerance.absolute),
    relative: String(settings.numeric.tolerance.relative),
    statistics: settings.missingValues.statistics,
  };
}

function draftSettings(
  draft: ProjectComputationSettingsDraft,
): ProjectComputationSettingsDto | null {
  const absolute = Number(draft.absolute);
  const relative = Number(draft.relative);
  if (
    !Number.isFinite(absolute) ||
    !Number.isFinite(relative) ||
    absolute < 0 ||
    relative < 0 ||
    (absolute === 0 && relative === 0)
  )
    return null;
  return {
    numeric: { tolerance: { absolute, relative } },
    missingValues: { statistics: draft.statistics },
  };
}

function settingsEqual(
  left: ProjectComputationSettingsDto,
  right: ProjectComputationSettingsDto,
): boolean {
  return (
    left.numeric.tolerance.absolute === right.numeric.tolerance.absolute &&
    left.numeric.tolerance.relative === right.numeric.tolerance.relative &&
    left.missingValues.statistics === right.missingValues.statistics
  );
}

export function useProjectComputationSettings() {
  const { t } = useTranslation();
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [confirmed, setConfirmed] = useState<ComputationSettingsSnapshotDto | null>(null);
  const [draft, replaceDraft] = useState<ProjectComputationSettingsDraft>(() =>
    toDraft(RECOMMENDED_PROJECT_COMPUTATION_SETTINGS),
  );
  const [isLoading, setIsLoading] = useState(false);
  const [isProjectChangeBlocked, setIsProjectChangeBlocked] = useState(false);
  const [isApplying, setIsApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const listener: AuthorityListener = (snapshot) => {
      if (snapshot.projectInstanceId !== projectInstanceId) return;
      setConfirmed((previous) => {
        if (previous && previous.settingsRevision >= snapshot.settingsRevision) return previous;
        replaceDraft((currentDraft) => {
          const previousSettings = previous?.settings;
          const currentSettings = draftSettings(currentDraft);
          return !previousSettings ||
            (currentSettings && settingsEqual(currentSettings, previousSettings))
            ? toDraft(snapshot.settings)
            : currentDraft;
        });
        return snapshot;
      });
    };
    authorityListeners.add(listener);
    return () => {
      authorityListeners.delete(listener);
    };
  }, [projectInstanceId]);

  const parsedDraft = useMemo(() => draftSettings(draft), [draft]);
  const validationError = useMemo(() => {
    const absolute = Number(draft.absolute);
    const relative = Number(draft.relative);
    if (!Number.isFinite(absolute) || !Number.isFinite(relative) || absolute < 0 || relative < 0) {
      return "Numeric tolerances must be finite and nonnegative.";
    }
    if (absolute === 0 && relative === 0) {
      return "Absolute and relative tolerances cannot both be zero.";
    }
    return null;
  }, [draft.absolute, draft.relative]);
  const isDirty = Boolean(
    confirmed && parsedDraft && !settingsEqual(parsedDraft, confirmed.settings),
  );
  const previousProjectRef = useRef<string | null | undefined>(undefined);
  const dirtyRef = useRef(isDirty);
  dirtyRef.current = isDirty;

  useEffect(() => {
    let disposed = false;
    const previousProject = previousProjectRef.current;
    previousProjectRef.current = projectInstanceId;

    const load = async () => {
      setError(null);
      if (
        previousProject !== undefined &&
        previousProject !== projectInstanceId &&
        dirtyRef.current
      ) {
        const discard = await uiStore.confirm({
          title: "Discard computation changes?",
          message: "Changing projects will discard your unapplied computation settings.",
          confirmText: "Discard",
          cancelText: "Keep Draft",
          type: "danger",
        });
        if (disposed) return;
        if (!discard) {
          setConfirmed(null);
          setIsLoading(false);
          setIsProjectChangeBlocked(true);
          return;
        }
      }

      setIsProjectChangeBlocked(false);
      setConfirmed(null);
      if (!projectInstanceId) {
        setIsLoading(false);
        replaceDraft(toDraft(RECOMMENDED_PROJECT_COMPUTATION_SETTINGS));
        return;
      }
      const identity = captureProjectIdentity();
      setIsLoading(true);
      try {
        const snapshot = await ProjectService.getProjectComputationSettings(projectInstanceId);
        if (
          disposed ||
          !isCurrentProjectIdentity(identity) ||
          snapshot.projectInstanceId !== identity.projectInstanceId
        )
          return;
        publish(snapshot);
        setConfirmed(snapshot);
        replaceDraft(toDraft(snapshot.settings));
      } catch (loadError) {
        if (!disposed && isCurrentProjectIdentity(identity)) {
          setError(formatInlineUserError(loadError, t));
        }
      } finally {
        if (!disposed && isCurrentProjectIdentity(identity)) setIsLoading(false);
      }
    };

    void load();
    return () => {
      disposed = true;
    };
  }, [projectInstanceId, t]);

  const setDraft = useCallback((patch: Partial<ProjectComputationSettingsDraft>) => {
    replaceDraft((current) => ({ ...current, ...patch }));
  }, []);

  const apply = useCallback(async () => {
    if (!projectInstanceId || !confirmed || !parsedDraft || validationError) return;
    const identity = captureProjectIdentity();
    const operationId = crypto.randomUUID();
    setIsApplying(true);
    setError(null);
    try {
      const receipt = await ProjectService.updateProjectComputationSettings({
        projectInstanceId: identity.projectInstanceId,
        operationId,
        expectedRevision: confirmed.settingsRevision,
        settings: parsedDraft,
      });
      if (!isCurrentProjectIdentity(identity)) return;
      if (
        receipt.projectInstanceId !== identity.projectInstanceId ||
        receipt.operationId !== operationId
      ) {
        throw new Error("Computation settings receipt correlation is invalid.");
      }
      if (receipt.settingsRevision !== confirmed.settingsRevision + 1) {
        throw new Error("Computation settings receipt revision is invalid.");
      }
      const latest = latestByProject.get(identity.projectInstanceId);
      if (latest && latest.settingsRevision > receipt.settingsRevision) return;
      publish(receipt);
      setConfirmed(receipt);
      replaceDraft(toDraft(receipt.settings));
    } catch (applyError) {
      if (isCurrentProjectIdentity(identity)) {
        setError(formatInlineUserError(applyError, t));
      }
      throw applyError;
    } finally {
      if (isCurrentProjectIdentity(identity)) setIsApplying(false);
    }
  }, [confirmed, parsedDraft, projectInstanceId, t, validationError]);

  const restoreRecommended = useCallback(() => {
    replaceDraft(toDraft(RECOMMENDED_PROJECT_COMPUTATION_SETTINGS));
  }, []);

  return {
    enabled: Boolean(projectInstanceId) && !isProjectChangeBlocked,
    confirmed,
    draft,
    isLoading,
    isApplying,
    isDirty,
    validationError,
    error,
    setDraft,
    apply,
    restoreRecommended,
  };
}
