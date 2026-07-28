import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscCopy, VscEdit, VscLink, VscTrash } from "react-icons/vsc";
import { ContextMenu, type ContextMenuPosition, type ContextMenuSection } from "@/shared/ui/contextMenu";
import type { NodeCapabilitiesDto } from '@/shared/types/dto/editorProjection';
import {
  EDITOR_MUTATION_CAPABILITIES,
  NODE_CREATION_UNAVAILABLE_MESSAGE,
} from '@/features/application/editor/editorMutationAvailability';

export interface NodeContextMenuProps {
  position: ContextMenuPosition;
  capabilities?: Pick<NodeCapabilitiesDto, 'managed' | 'canCopy' | 'canDelete'>;
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

  const sections = useMemo((): ContextMenuSection[] => {
    const n = (key: string) => t(`contextMenu.node.${key}`);
    return [
      {
        items: [
          { id: "copy", label: n("copy"), icon: <VscCopy size={12} />, disabled: !canCopy, shortcut: "Ctrl+C", onClick: onCopy },
          { id: "cut", label: n("cut"), icon: <VscCopy size={12} />, disabled: !canCut, shortcut: "Ctrl+X", onClick: onCut },
          {
            id: "duplicate",
            label: n("duplicate"),
            icon: <VscCopy size={12} />,
            disabled: !EDITOR_MUTATION_CAPABILITIES.duplicateNodes,
            title: NODE_CREATION_UNAVAILABLE_MESSAGE,
            shortcut: "Ctrl+D",
            onClick: onDuplicate,
          },
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
          { id: "delete", label: n("delete"), icon: <VscTrash size={12} />, disabled: !canDelete, danger: true, shortcut: "Del", onClick: onDelete },
        ],
      },
    ];
  }, [t, canCopy, canCut, canDelete, hasLinks, onCopy, onCut, onDuplicate, onDelete, onBreakAllLinks, onSelectLinked]);

  return (
    <ContextMenu
      position={position}
      sections={sections}
      onClose={onClose}
    />
  );
};
