import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { settingsUi } from "@/features/core/settings/ui";
import { ui } from "@/features/core/ui/ui";
import { formatInlineUserError } from "@/features/application/userErrorSummary";

type SettingsSection = "appearance";
type ResetTarget = SettingsSection | "all";

export function useSettingsReset(modalId: string) {
  const { t } = useTranslation();
  const mounted = useRef(false);
  const pending = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const [phase, setPhase] = useState<"idle" | "confirming" | "saving">("idle");
  const [resetAllError, setResetAllError] = useState<string | null>(null);
  const [sectionResetError, setSectionResetError] = useState<{
    section: SettingsSection;
    message: string;
  } | null>(null);

  const resetSettings = async (target: ResetTarget) => {
    if (!mounted.current || pending.current) return;
    pending.current = true;
    const sectionName = target === "all" ? null : t(`settings.sections.${target}`);
    setPhase("confirming");
    try {
      const confirmed = await ui.confirm(
        {
          title: t(
            target === "all" ? "settings.confirmResetAllTitle" : "settings.confirmResetTitle",
          ),
          message:
            target === "all"
              ? t("settings.confirmResetAllMessage")
              : t("settings.confirmResetMessage", { section: sectionName }),
          type: "danger",
          confirmText: t("common.restoreDefaults"),
        },
        modalId,
      );
      if (!confirmed || !mounted.current) return;

      setPhase("saving");
      if (target === "all") {
        setResetAllError(null);
        setSectionResetError(null);
        await settingsUi.resetAllToDefaults();
      } else {
        setSectionResetError((current) => (current?.section === target ? null : current));
        await settingsUi.resetAppearanceToDefaults();
      }
    } catch (error) {
      if (!mounted.current) return;
      if (target === "all") {
        setResetAllError(
          t("settings.restoreAllFailed", { error: formatInlineUserError(error, t) }),
        );
      } else {
        setSectionResetError({
          section: target,
          message: t("settings.restoreSectionFailed", {
            section: sectionName,
            error: formatInlineUserError(error, t),
          }),
        });
      }
    } finally {
      pending.current = false;
      if (mounted.current) setPhase("idle");
    }
  };

  return {
    resetSettings,
    isResetPending: phase !== "idle",
    isResetting: phase === "saving",
    resetAllError,
    sectionResetError,
  };
}
