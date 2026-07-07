import React from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { LayoutTab } from "@/shared/types";
import { useEditorGroup } from "@/features/application/editor";

import { useCanvasOverlayHandlers, type VariableDropMenu } from "@/features/application/editor";
import { ContextMenu } from "@/shared/ui/contextMenu";
import { NodePalette, type PaletteItem } from "../../Layout/NodePalette";
import { PinResultSearch } from "./PinResultSearchPalette";
import { CanvasExecutionToolbar } from "./CanvasExecutionToolbar";

export default function CanvasOverlays({
    canvasElementRef,
    variableDropMenu,
    setVariableDropMenu,
    onVariableDropGet,
    onVariableDropSet,
}: {
    canvasElementRef: React.RefObject<HTMLDivElement | null>;
    variableDropMenu: VariableDropMenu | null;
    setVariableDropMenu: (val: VariableDropMenu | null) => void;
    onVariableDropGet: (menu: VariableDropMenu) => void | Promise<void>;
    onVariableDropSet: (menu: VariableDropMenu) => void | Promise<void>;
}) {
    const { t } = useTranslation();
    const {
        contextMenu,
        setContextMenu,
        setPendingConnection,
        pendingConnection,
        variables,
        functions,
        tabs,
        activeTabId,
        activeGroupId,
        groupId,
        executeGraph,
        cancelGraphExecution,
        clearGraphArtifacts,
        setCanvas,
        createNode,
    } = useEditorGroup();

    const {
        handleNodePaletteSelect,
    } = useCanvasOverlayHandlers({
        canvasElementRef,
        activeTabId,
        functions,
        pendingConnection,
        setContextMenu,
        setPendingConnection,
        createNode,
        setCanvas,
    });

    const tabId = activeTabId ?? "";
    const onPaletteSelect = (item: PaletteItem) =>
        contextMenu && handleNodePaletteSelect(item, contextMenu);

    const activeTabType = tabs.find((t: LayoutTab) => t.id === activeTabId)?.type;
    const isEventTab = activeTabType === "event";
    const graphKind: "event" | "function" | undefined =
        activeTabType === "event" ? "event" : activeTabType === "function" ? "function" : undefined;

    return (
        <>
            {isEventTab && (
                <div className="absolute left-3 top-3 z-40">
                    <PinResultSearch graphId={tabId} />
                </div>
            )}

            {isEventTab && (
                <CanvasExecutionToolbar
                    graphId={tabId}
                    onExecute={() => executeGraph(tabId)}
                    onCancelExecution={() => void cancelGraphExecution()}
                    onClearArtifacts={() => void clearGraphArtifacts(tabId)}
                />
            )}

            {/* ================= Node Palette ================= */}
            {activeGroupId === groupId && contextMenu?.visible && createPortal(
                <div className="menu-container">
                    <NodePalette
                        x={contextMenu.x}
                        y={contextMenu.y}
                        onSelect={onPaletteSelect}
                        filterPin={pendingConnection}
                        variables={variables}
                        functions={functions}
                        graphKind={graphKind}
                        graphId={activeTabId ?? undefined}
                    />
                </div>,
                document.body
            )}

            {/* ================= Variable Drop Menu ================= */}
            {activeGroupId === groupId && variableDropMenu && (
                <ContextMenu
                    position={{ x: variableDropMenu.x, y: variableDropMenu.y }}
                    sections={[
                        {
                            items: [
                                {
                                    id: "get-variable",
                                    label: t("canvas.getVariable", { name: variableDropMenu.variableName }),
                                    onClick: () => void onVariableDropGet(variableDropMenu),
                                },
                            ],
                        },
                        {
                            items: [
                                {
                                    id: "set-variable",
                                    label: t("canvas.setVariable", { name: variableDropMenu.variableName }),
                                    onClick: () => void onVariableDropSet(variableDropMenu),
                                },
                            ],
                        },
                    ]}
                    onClose={() => setVariableDropMenu(null)}
                />
            )}
        </>
    );
}
