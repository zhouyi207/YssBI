import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscEye, VscLink, VscRefresh } from "react-icons/vsc";
import {
  ActionMenu,
  type ActionMenuPosition,
  type ActionMenuSection,
  type ActionMenuItem,
} from "@/shared/ui/actionMenu";

export interface PinContextMenuProps {
  position: ActionMenuPosition;
  hasLinks?: boolean;
  canReset?: boolean;
  onBreakLinks?: () => void;
  onResetValue?: () => void;
  showView?: boolean;
  onView?: () => void;
  onClose: () => void;
}

export const PinContextMenu: React.FC<PinContextMenuProps> = ({
  position,
  hasLinks,
  canReset,
  onBreakLinks,
  onResetValue,
  showView = false,
  onView,
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
        onClick: onView,
      });
    }

    return [{ items: primaryItems }];
  }, [t, hasLinks, canReset, onBreakLinks, onResetValue, showView, onView]);

  return <ActionMenu position={position} sections={sections} onClose={onClose} />;
};
