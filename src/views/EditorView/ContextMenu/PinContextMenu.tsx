import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection } from "./ContextMenu";

interface PinContextMenuProps {
  position: ContextMenuPosition;
  removable?: boolean;
  onRemove?: () => void;
  onClose: () => void;
}

export const PinContextMenu: React.FC<PinContextMenuProps> = ({
  position,
  removable,
  onRemove,
  onClose,
}) => {
  const { t } = useTranslation();

  const sections = useMemo((): ContextMenuSection[] => {
    const p = (key: string) => t(`contextMenu.pin.${key}`);
    return [
      {
        items: [
          { id: "breakLinks", label: p("breakLinks"), disabled: true },
          { id: "resetValue", label: p("resetValue"), disabled: true },
          { id: "promoteToVar", label: p("promoteToVar"), disabled: true },
        ],
      },
      {
        items: [
          {
            id: "removePin",
            label: p("removePin"),
            danger: true,
            disabled: !removable,
            onClick: onRemove,
          },
        ],
      },
    ];
  }, [t, removable, onRemove]);

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
