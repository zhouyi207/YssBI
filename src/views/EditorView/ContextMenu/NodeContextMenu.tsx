import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection } from "./ContextMenu";

interface NodeContextMenuProps {
  position: ContextMenuPosition;
  onClose: () => void;
}

export const NodeContextMenu: React.FC<NodeContextMenuProps> = ({
  position,
  onClose,
}) => {
  const { t } = useTranslation();

  const sections = useMemo((): ContextMenuSection[] => {
    const n = (key: string) => t(`contextMenu.node.${key}`);
    return [
      {
        items: [
          { id: "copy", label: n("copy"), disabled: true, shortcut: "Ctrl+C" },
          { id: "cut", label: n("cut"), disabled: true, shortcut: "Ctrl+X" },
          { id: "duplicate", label: n("duplicate"), disabled: true, shortcut: "Ctrl+D" },
        ],
      },
      {
        items: [
          { id: "disable", label: n("disableNode"), disabled: true },
          { id: "rename", label: n("rename"), disabled: true, shortcut: "F2" },
          { id: "collapse", label: n("collapse"), disabled: true },
        ],
      },
      {
        items: [
          { id: "breakLinks", label: n("breakAllLinks"), disabled: true },
          { id: "selectLinked", label: n("selectLinkedNodes"), disabled: true },
        ],
      },
      {
        items: [
          { id: "delete", label: n("delete"), disabled: true, danger: true, shortcut: "Del" },
        ],
      },
    ];
  }, [t]);

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
