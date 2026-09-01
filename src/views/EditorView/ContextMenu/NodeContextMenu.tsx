import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscCopy, VscLink, VscTrash } from "react-icons/vsc";
import {
  ActionMenu,
  type ActionMenuPosition,
  type ActionMenuSection,
} from "@/shared/ui/actionMenu";
import type { NodeCapabilitiesDto } from "@/shared/types/domain/editorProjection";

export interface NodeContextMenuProps {
  position: ActionMenuPosition;
  capabilities?: Pick<NodeCapabilitiesDto, "managed" | "canCopy" | "canDelete">;
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
  capabilities,
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
  const canCopy = capabilities?.managed === false && capabilities.canCopy === true;
  const canDelete = capabilities?.managed === false && capabilities.canDelete === true;
  const canCut = canCopy && canDelete;

  const sections = useMemo((): ActionMenuSection[] => {
    const n = (key: string) => t(`contextMenu.node.${key}`);
    return [
      {
        items: [
          {
            id: "copy",
            label: n("copy"),
            icon: <VscCopy size={12} />,
            disabled: !canCopy,
            shortcut: "Ctrl+C",
            onClick: onCopy,
          },
          {
            id: "cut",
            label: n("cut"),
            icon: <VscCopy size={12} />,
            disabled: !canCut,
            shortcut: "Ctrl+X",
            onClick: onCut,
          },
          {
            id: "duplicate",
            label: n("duplicate"),
            icon: <VscCopy size={12} />,
            disabled: !canCopy,
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
            disabled: !canDelete,
            danger: true,
            shortcut: "Del",
            onClick: onDelete,
          },
        ],
      },
    ];
  }, [
    t,
    canCopy,
    canCut,
    canDelete,
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
