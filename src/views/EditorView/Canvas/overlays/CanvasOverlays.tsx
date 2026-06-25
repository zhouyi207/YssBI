import React from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { LayoutTab } from "@/shared/types";
import { useEditorGroup } from "@/features/application/editor";
import { useGestureStore } from "@/features/core/gesture";
import { useExecutionPlayback, useExecutionStore } from "@/features/core/execution";

import { useNodeManagement } from "@/features/application/dataManagement";
import { useCanvasOverlayHandlers } from "@/features/application/editor";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ContextMenu } from "@/shared/ui/contextMenu";
import { NodePalette, type PaletteItem } from "../../Layout/NodePalette";
import {
  VscDebugStart,
  VscDebugPause,
  VscDebugStop,
  VscDebugRestart,
  VscPlay,
  VscRunAll,
} from "react-icons/vsc";

function CanvasToolbarButton({
    tooltip,
    children,
    ...props
}: React.ComponentProps<typeof Button> & { tooltip: string }) {
    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <Button {...props}>{children}</Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{tooltip}</TooltipContent>
        </Tooltip>
    );
}

function SelectionRegion({
    groupId,
    canvasRef,
}: {
    groupId: string;
    canvasRef: React.RefObject<HTMLDivElement | null>;
}) {
    const gesture = useGestureStore((state) => {
        const current = state.gesture;
        return current?.type === "select" ? current : null;
    });

    if (gesture?.groupId !== groupId || !canvasRef.current) return null;

    const canvasBounds = canvasRef.current.getBoundingClientRect();

    return (
        <div
            className="absolute pointer-events-none z-50 border-2 border-dashed border-[var(--accent-color)] bg-[var(--selection-region)]/15"
            style={{
                left: Math.min(gesture.startX, gesture.currentX) - canvasBounds.left,
                top: Math.min(gesture.startY, gesture.currentY) - canvasBounds.top,
                width: Math.abs(gesture.startX - gesture.currentX),
                height: Math.abs(gesture.startY - gesture.currentY),
            }}
        />
    );
}

export default function CanvasOverlays({
    canvasRef,
    variableDropMenu,
    setVariableDropMenu
}: {
    canvasRef: React.RefObject<HTMLDivElement | null>;
    variableDropMenu: any;
    setVariableDropMenu: (val: any) => void;
}) {
    const { t } = useTranslation();
    const {
        contextMenu,
        setContextMenu,
        setPendingConnection,
        pendingConnection,
        variables,
        Variables,
        functions,
        tabs,
        activeTabId,
        activeGroupId,
        groupId,
        executeGraph,
        setCanvas,
    } = useEditorGroup();

    const { createNode } = useNodeManagement();

    const {
        handleNodePaletteSelect,
        handleVariableDropGet,
        handleVariableDropSet,
    } = useCanvasOverlayHandlers({
        canvasRef,
        groupId,
        activeTabId,
        functions,
        variables,
        Variables,
        pendingConnection,
        setContextMenu,
        setPendingConnection,
        setVariableDropMenu,
        createNode,
        setCanvas,
    });

    const tabId = activeTabId ?? "";
    const { stop, togglePlayPause, isPlaying, isPaused, hasRecording, graphDirty } = useExecutionPlayback(tabId);
    const graphStatus = useExecutionStore((s) => s.graphs[tabId]?.status ?? "idle");

    const playbackActive = isPlaying || isPaused;
    const isThisGraphRunning = graphStatus === "running";
    const canReplay = hasRecording && !graphDirty && !isThisGraphRunning;

    const onPaletteSelect = (item: PaletteItem) =>
        contextMenu && handleNodePaletteSelect(item, contextMenu);

    const isEventTab = tabs.find((t: LayoutTab) => t.id === activeTabId)?.type === "event";

    return (
        <>
            {isEventTab && (
                <div className="absolute top-3 right-3 z-40 flex items-center gap-1 bg-[var(--panel-bg)]/80 backdrop-blur-sm border border-[var(--border-color)] rounded-md p-0.5 shadow-lg">
                    {/* Debug placeholder */}
                    <CanvasToolbarButton
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled
                        className="text-[var(--text-secondary)] opacity-40"
                        tooltip={t("canvas.debugComingSoon")}
                    >
                        <VscDebugStart size={14} />
                    </CanvasToolbarButton>

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    {!playbackActive ? (
                        <CanvasToolbarButton
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => canReplay && togglePlayPause()}
                            disabled={!canReplay}
                            className={
                                canReplay
                                    ? 'text-blue-400 hover:text-blue-300'
                                    : 'text-[var(--text-secondary)] opacity-40 cursor-not-allowed'
                            }
                            tooltip={
                                graphDirty ? t("canvas.replayDisabledDirty") :
                                !hasRecording ? t("canvas.replayNoRecording") :
                                t("canvas.replayExecution")
                            }
                        >
                            <VscDebugRestart size={14} />
                        </CanvasToolbarButton>
                    ) : (
                        <div className="flex items-center">
                            <CanvasToolbarButton
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={togglePlayPause}
                                className={isPlaying ? 'text-amber-400' : 'text-blue-400'}
                                tooltip={isPlaying ? t("canvas.pauseReplay") : t("canvas.resumeReplay")}
                            >
                                {isPlaying ? <VscDebugPause size={14} /> : <VscPlay size={14} />}
                            </CanvasToolbarButton>
                            <CanvasToolbarButton
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={stop}
                                className="text-red-400"
                                tooltip={t("canvas.stopReplay")}
                            >
                                <VscDebugStop size={14} />
                            </CanvasToolbarButton>
                        </div>
                    )}

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    <CanvasToolbarButton
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => !isThisGraphRunning && executeGraph(tabId)}
                        disabled={isThisGraphRunning}
                        className={
                            isThisGraphRunning
                                ? 'text-green-400 opacity-60 cursor-not-allowed'
                                : 'text-green-400 hover:text-green-300'
                        }
                        tooltip={isThisGraphRunning ? t("canvas.executing") : t("canvas.executeCurrentEvent")}
                    >
                        <VscRunAll size={14} />
                    </CanvasToolbarButton>
                </div>
            )}

            {/* ================= Selection Box ================= */}
            <SelectionRegion groupId={groupId} canvasRef={canvasRef} />

            {/* ================= Node Palette ================= */}
            {activeGroupId === groupId && contextMenu?.visible && createPortal(
                <div className="menu-container">
                    <NodePalette
                        x={contextMenu.x}
                        y={contextMenu.y}
                        onSelect={onPaletteSelect}
                        filterPin={pendingConnection}
                        variables={variables}
                        Variables={Variables}
                        functions={functions}
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
                                    onClick: () => handleVariableDropGet(variableDropMenu),
                                },
                            ],
                        },
                        {
                            items: [
                                {
                                    id: "set-variable",
                                    label: t("canvas.setVariable", { name: variableDropMenu.variableName }),
                                    onClick: () => handleVariableDropSet(variableDropMenu),
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
