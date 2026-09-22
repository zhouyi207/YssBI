import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscCopy, VscLink, VscTrash } from "react-icons/vsc";
import {
  ActionMenu,
  type ActionMenuPosition,
  type ActionMenuSection,
} from "@/shared/ui/actionMenu";

export interface NodeContextMenuProps {
  position: ActionMenuPosition;
  managed?: boolean;
  hasLinks?: boolean;
  onCopy: () => void;
  onCut: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onBreakAllLinks: () => void;
  onSelectLinked: () => void;
  onClose: () => void;
}

export const NodeContextMenu: React.FC<NodeContextMenuProps> = ({
  position,
  managed,
  hasLinks,
  onCopy,
  onCut,
  onDuplicate,
  onDelete,
  onBreakAllLinks,
  onSelectLinked,
  onClose,
}) => {
  const { t } = useTranslation();
  const canModify = managed === false;

  const sections = useMemo((): ActionMenuSection[] => {
    const n = (key: string) => t(`contextMenu.node.${key}`);
    return [
      {
        items: [
          {
            id: "copy",
            label: n("copy"),
            icon: <VscCopy size={12} />,
            disabled: !canModify,
            shortcut: "Ctrl+C",
            onClick: onCopy,
          },
          {
            id: "cut",
            label: n("cut"),
            icon: <VscCopy size={12} />,
            disabled: !canModify,
            shortcut: "Ctrl+X",
            onClick: onCut,
          },
          {
            id: "duplicate",
            label: n("duplicate"),
            icon: <VscCopy size={12} />,
            disabled: !canModify,
            shortcut: "Ctrl+D",
            onClick: onDuplicate,
          },
        ],
      },
      {
        items: [
          {
            id: "breakLinks",
            label: n("breakAllLinks"),
            icon: <VscLink size={12} />,
            disabled: !hasLinks,
            onClick: onBreakAllLinks,
          },
          {
            id: "selectLinked",
            label: n("selectLinkedNodes"),
            icon: <VscLink size={12} />,
            disabled: !hasLinks,
            onClick: onSelectLinked,
          },
        ],
      },
      {
        items: [
          {
            id: "delete",
            label: n("delete"),
            icon: <VscTrash size={12} />,
            disabled: !canModify,
            danger: true,
            shortcut: "Del",
            onClick: onDelete,
          },
        ],
      },
    ];
  }, [
    t,
    canModify,
    hasLinks,
    onCopy,
    onCut,
    onDuplicate,
    onDelete,
    onBreakAllLinks,
    onSelectLinked,
  ]);

  return <ActionMenu position={position} sections={sections} onClose={onClose} />;
};
