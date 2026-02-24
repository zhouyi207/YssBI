import React from "react";
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
  const sections: ContextMenuSection[] = [
    {
      items: [
        { id: "breakLinks", label: "Break Links", disabled: true },
        { id: "resetValue", label: "Reset to Default", disabled: true },
        { id: "promoteToVar", label: "Promote to Variable", disabled: true },
      ],
    },
    {
      items: [
        {
          id: "removePin",
          label: "Remove Pin",
          danger: true,
          disabled: !removable,
          onClick: onRemove,
        },
      ],
    },
  ];

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
