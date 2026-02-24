import React from "react";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection } from "./ContextMenu";

interface NodeContextMenuProps {
  position: ContextMenuPosition;
  nodeTitle: string;
  onClose: () => void;
}

const NODE_MENU_SECTIONS: ContextMenuSection[] = [
  {
    items: [
      { id: "copy", label: "Copy", disabled: true, shortcut: "Ctrl+C" },
      { id: "cut", label: "Cut", disabled: true, shortcut: "Ctrl+X" },
      { id: "duplicate", label: "Duplicate", disabled: true, shortcut: "Ctrl+D" },
    ],
  },
  {
    items: [
      { id: "disable", label: "Disable Node", disabled: true },
      { id: "rename", label: "Rename", disabled: true, shortcut: "F2" },
      { id: "collapse", label: "Collapse", disabled: true },
    ],
  },
  {
    items: [
      { id: "breakLinks", label: "Break All Links", disabled: true },
      { id: "selectLinked", label: "Select Linked Nodes", disabled: true },
    ],
  },
  {
    items: [
      { id: "delete", label: "Delete", disabled: true, danger: true, shortcut: "Del" },
    ],
  },
];

export const NodeContextMenu: React.FC<NodeContextMenuProps> = ({
  position,
  onClose,
}) => {
  return (
    <ContextMenu
      position={position}
      sections={NODE_MENU_SECTIONS}
      onClose={onClose}
    />
  );
};
