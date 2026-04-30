import { VscAdd, VscChevronRight, VscCopy, VscEdit, VscNewFolder, VscTrash } from "react-icons/vsc";
import type { ContextMenuSection } from "@/shared/ui/contextMenu";
import type { SidebarContextMenuActions, SidebarContextMenuState } from "./sidebarContextMenuTypes";

export function buildSidebarContextMenuSections(
  contextMenu: SidebarContextMenuState | null,
  actions: SidebarContextMenuActions
): ContextMenuSection[] {
  if (!contextMenu) return [];
  const target = contextMenu.target;

  if (target.type === "graph") {
    return [
      {
        items: [
          { id: "open", label: "Open", icon: <VscChevronRight size={12} />, onClick: () => actions.openGraph(target.id, target.name, target.graphType) },
          { id: "rename", label: "Rename", icon: <VscEdit size={12} />, onClick: () => actions.renameGraphItem(target.id, target.name, target.graphType) },
          { id: "duplicate", label: "Duplicate", icon: <VscCopy size={12} />, onClick: () => void actions.duplicateGraphItem(target.id) },
        ],
      },
      {
        items: [
          { id: "delete", label: "Delete", icon: <VscTrash size={12} />, danger: true, onClick: () => void actions.deleteGraphItem(target.id, target.graphType) },
        ],
      },
    ];
  }

  if (target.type === "folder") {
    return [
      {
        items: [
          { id: "new-graph", label: target.graphType === "event" ? "New Event" : "New Function", icon: <VscAdd size={12} />, onClick: () => void actions.createGraphInFolder(target.graphType, target.folderPath) },
          { id: "new-folder", label: "New Folder", icon: <VscNewFolder size={12} />, onClick: () => actions.createFolderInFolder(target.graphType, target.folderPath) },
          { id: "rename-folder", label: "Rename Folder", icon: <VscEdit size={12} />, onClick: () => actions.renameFolderItem(target.graphType, target.folderPath, target.name) },
        ],
      },
      {
        items: [
          { id: "delete-folder", label: "Delete Folder", icon: <VscTrash size={12} />, danger: true, onClick: () => void actions.deleteFolderItem(target.graphType, target.folderPath) },
        ],
      },
    ];
  }

  if (target.type === "section") {
    return [
      {
        items: [
          { id: "new-graph", label: target.graphType === "event" ? "New Event" : "New Function", icon: <VscAdd size={12} />, onClick: () => void actions.createGraphInFolder(target.graphType, target.folderPath ?? "") },
          { id: "new-folder", label: "New Folder", icon: <VscNewFolder size={12} />, onClick: () => actions.createFolderInFolder(target.graphType, target.folderPath ?? "") },
        ],
      },
    ];
  }

  return [
    {
      items: [
        { id: "new-variable", label: "New Variable", icon: <VscAdd size={12} />, onClick: () => void actions.addVariable("New Variable", "Int32", false) },
        { id: "rename-variable", label: "Rename", icon: <VscEdit size={12} />, onClick: () => actions.renameVariableItem(target.id, target.name) },
      ],
    },
    {
      items: [
        { id: "delete-variable", label: "Delete", icon: <VscTrash size={12} />, danger: true, onClick: () => void actions.deleteVariable(target.id) },
      ],
    },
  ];
}
