import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscEye, VscLink, VscRefresh, VscSymbolVariable, VscTrash } from "react-icons/vsc";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection, type ContextMenuItem } from "./ContextMenu";

export interface PinContextMenuProps {
  position: ContextMenuPosition;
  removable?: boolean;
  hasLinks?: boolean;
  canReset?: boolean;
  onBreakLinks?: () => void;
  onResetValue?: () => void;
  showView?: boolean;
  viewEnabled?: boolean;
  viewDisabledTitle?: string;
  onView?: () => void;
  onRemove?: () => void;
  onClose: () => void;
}

export const PinContextMenu: React.FC<PinContextMenuProps> = ({
  position,
  removable,
  hasLinks,
  canReset,
  onBreakLinks,
  onResetValue,
  showView = false,
  viewEnabled = false,
  viewDisabledTitle,
  onView,
  onRemove,
  onClose,
}) => {
  const { t } = useTranslation();

  const sections = useMemo((): ContextMenuSection[] => {
    const p = (key: string) => t(`contextMenu.pin.${key}`);
    const primaryItems: ContextMenuItem[] = [
      { id: "breakLinks", label: p("breakLinks"), icon: <VscLink size={12} />, disabled: !hasLinks, onClick: onBreakLinks },
      { id: "resetValue", label: p("resetValue"), icon: <VscRefresh size={12} />, disabled: !canReset, onClick: onResetValue },
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

    primaryItems.push({
      id: "promoteToVar",
      label: p("promoteToVar"),
      icon: <VscSymbolVariable size={12} />,
      disabled: true,
      onClick: undefined,
    });

    return [
      { items: primaryItems },
      {
        items: [
          {
            id: "removePin",
            label: p("removePin"),
            icon: <VscTrash size={12} />,
            danger: true,
            disabled: !removable,
            onClick: onRemove,
          },
        ],
      },
    ];
  }, [
    t,
    removable,
    hasLinks,
    canReset,
    onBreakLinks,
    onResetValue,
    showView,
    viewEnabled,
    viewDisabledTitle,
    onView,
    onRemove,
  ]);

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
