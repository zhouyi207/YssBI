import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscEye, VscLink, VscRefresh } from "react-icons/vsc";
import {
  ActionMenu,
  type ActionMenuPosition,
  type ActionMenuSection,
  type ActionMenuItem,
} from "@/shared/ui/actionMenu";
import type { PinResultEntry } from "@/shared/types/domain/result";

export interface PinContextMenuProps {
  position: ActionMenuPosition;
  hasLinks?: boolean;
  canReset?: boolean;
  onBreakLinks?: () => void;
  onResetValue?: () => void;
  showView?: boolean;
  viewEnabled?: boolean;
  viewDisabledTitle?: string;
  onView?: () => void;
  historyEntries?: readonly PinResultEntry[];
  onViewHistory?: (resultId: string) => void;
  onClose: () => void;
}

export const PinContextMenu: React.FC<PinContextMenuProps> = ({
  position,
  hasLinks,
  canReset,
  onBreakLinks,
  onResetValue,
  showView = false,
  viewEnabled = false,
  viewDisabledTitle,
  onView,
  historyEntries,
  onViewHistory,
  onClose,
}) => {
  const { t } = useTranslation();

  const sections = useMemo((): ActionMenuSection[] => {
    const p = (key: string) => t(`contextMenu.pin.${key}`);
    const primaryItems: ActionMenuItem[] = [
      {
        id: "breakLinks",
        label: p("breakLinks"),
        icon: <VscLink size={12} />,
        disabled: !hasLinks,
        onClick: onBreakLinks,
      },
      {
        id: "resetValue",
        label: p("resetValue"),
        icon: <VscRefresh size={12} />,
        disabled: !canReset,
        onClick: onResetValue,
      },
    ];

    if (showView) {
      primaryItems.push({
        id: "view",
        label: p("view"),
        icon: <VscEye size={12} />,
        disabled: !viewEnabled,
        title: viewEnabled ? undefined : viewDisabledTitle,
        onClick: onView,
      });
    }

    [...(historyEntries ?? [])].reverse().forEach((entry, index) => {
      const createdAt = Number(entry.createdAtMs);
      const time = Number.isFinite(createdAt)
        ? new Date(createdAt).toLocaleString()
        : entry.createdAtMs;
      primaryItems.push({
        id: `view-history-${entry.activationId}`,
        label: `${entry.resultId} · ${entry.state.kind}${index === 0 ? ` · ${p("historyLatest")}` : ""}`,
        shortcut: `${time} · ${entry.runId}`,
        onClick: () => onViewHistory?.(entry.resultId),
      });
    });

    return [{ items: primaryItems }];
  }, [
    t,
    hasLinks,
    canReset,
    onBreakLinks,
    onResetValue,
    showView,
    viewEnabled,
    viewDisabledTitle,
    onView,
    historyEntries,
    onViewHistory,
  ]);

  return <ActionMenu position={position} sections={sections} onClose={onClose} />;
};
