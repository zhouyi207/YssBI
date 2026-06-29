import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscEye, VscLink, VscRefresh, VscSymbolVariable, VscTrash } from "react-icons/vsc";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection } from "./ContextMenu";

export interface PinContextMenuProps {
  position: ContextMenuPosition;
  removable?: boolean;
  hasLinks?: boolean;
  canReset?: boolean;
  onBreakLinks?: () => void;
  onResetValue?: () => void;
  onInspectResult?: () => void;
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
  onInspectResult,
  onRemove,
  onClose,
}) => {
  const { t } = useTranslation();

  const sections = useMemo((): ContextMenuSection[] => {
    const p = (key: string) => t(`contextMenu.pin.${key}`);
    return [
      {
        items: [
          { id: "breakLinks", label: p("breakLinks"), icon: <VscLink size={12} />, disabled: !hasLinks, onClick: onBreakLinks },
          { id: "resetValue", label: p("resetValue"), icon: <VscRefresh size={12} />, disabled: !canReset, onClick: onResetValue },
          { id: "inspectResult", label: "Inspect result", icon: <VscEye size={12} />, disabled: !onInspectResult, onClick: onInspectResult },
          { id: "promoteToVar", label: p("promoteToVar"), icon: <VscSymbolVariable size={12} />, disabled: true },
        ],
      },
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
  }, [t, removable, hasLinks, canReset, onBreakLinks, onResetValue, onInspectResult, onRemove]);

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
