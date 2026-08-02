import React from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { LayoutTab } from "@/shared/types";
import { useEditorGroup } from "@/features/application/editor";
import { getOverlayPortalRoot } from "@/shared/ui/overlayPortalRoot";

import { useCanvasOverlayHandlers, type VariableDropMenu } from "@/features/application/editor";
import { ContextMenu } from "@/shared/ui/contextMenu";
import { NodePalette } from "../../Layout/NodePalette";
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
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
        tabs,
        activeTabId,
        groupId,
        executeGraph,
        cancelGraphExecution,
        clearGraphArtifacts,
    } = useEditorGroup({ withCanvasUi: true });

    const {
        handleNodePaletteSelect,
    } = useCanvasOverlayHandlers({
        canvasElementRef,
        groupId,
        activeTabId,
        pendingConnection,
        setContextMenu,
        setPendingConnection,
    });

    const tabId = activeTabId ?? "";
    const onPaletteSelect = (descriptor: NodeCreationDescriptor, locale: string) => {
        if (contextMenu) void handleNodePaletteSelect(descriptor, locale, contextMenu);
    };

    const activeTabType = tabs.find((t: LayoutTab) => t.id === activeTabId)?.type;
    const isEventTab = activeTabType === "event";

    return (
        <>
            {isEventTab && (
                <div className="absolute left-3 top-3 z-40">
                    <PinResultSearch graphPath={tabId} />
                </div>
            )}

            {isEventTab && (
                <CanvasExecutionToolbar
                    graphPath={tabId}
                    onExecute={() => executeGraph(tabId)}
                    onCancelExecution={() => void cancelGraphExecution(tabId)}
                    onClearArtifacts={() => void clearGraphArtifacts(tabId)}
                />
            )}

            {/* ================= Node Palette ================= */}
            {contextMenu?.visible && createPortal(
                <div className="menu-container">
                    <NodePalette
                        x={contextMenu.x}
                        y={contextMenu.y}
                        onSelect={onPaletteSelect}
                    />
                </div>,
                getOverlayPortalRoot(),
            )}

            {/* ================= Variable Drop Menu ================= */}
            {variableDropMenu && (
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
