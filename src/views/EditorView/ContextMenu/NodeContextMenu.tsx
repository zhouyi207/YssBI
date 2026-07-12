import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscCopy, VscEdit, VscLink, VscTrash } from "react-icons/vsc";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection } from "@/shared/ui/contextMenu";

export interface NodeContextMenuProps {
  position: ContextMenuPosition;
  isInternal?: boolean;
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
  isInternal,
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
  const editable = !isInternal;

  const sections = useMemo((): ContextMenuSection[] => {
    const n = (key: string) => t(`contextMenu.node.${key}`);
    return [
      {
        items: [
          { id: "copy", label: n("copy"), icon: <VscCopy size={12} />, disabled: !editable, shortcut: "Ctrl+C", onClick: onCopy },
          { id: "cut", label: n("cut"), icon: <VscCopy size={12} />, disabled: !editable, shortcut: "Ctrl+X", onClick: onCut },
          { id: "duplicate", label: n("duplicate"), icon: <VscCopy size={12} />, disabled: !editable, shortcut: "Ctrl+D", onClick: onDuplicate },
        ],
      },
      {
        items: [
          { id: "disable", label: n("disableNode"), icon: <VscEdit size={12} />, disabled: true },
          { id: "rename", label: n("rename"), icon: <VscEdit size={12} />, disabled: true, shortcut: "F2" },
          { id: "collapse", label: n("collapse"), icon: <VscEdit size={12} />, disabled: true },
        ],
      },
      {
        items: [
          { id: "breakLinks", label: n("breakAllLinks"), icon: <VscLink size={12} />, disabled: !hasLinks, onClick: onBreakAllLinks },
          { id: "selectLinked", label: n("selectLinkedNodes"), icon: <VscLink size={12} />, disabled: !hasLinks, onClick: onSelectLinked },
        ],
      },
      {
        items: [
          { id: "delete", label: n("delete"), icon: <VscTrash size={12} />, disabled: !editable, danger: true, shortcut: "Del", onClick: onDelete },
        ],
      },
    ];
  }, [t, editable, hasLinks, onCopy, onCut, onDuplicate, onDelete, onBreakAllLinks, onSelectLinked]);

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
