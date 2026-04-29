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
import { Card } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { NodePalette, type PaletteItem } from "../../Layout/NodePalette";
import {
  VscDebugStart,
  VscDebugPause,
  VscDebugStop,
  VscDebugRestart,
  VscPlay,
  VscRunAll,
} from "react-icons/vsc";

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
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled
                        className="text-[var(--text-secondary)] opacity-40"
                        title={t("canvas.debugComingSoon")}
                    >
                        <VscDebugStart size={14} />
                    </Button>

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    {/* Replay */}
                    {!playbackActive ? (
                        <Button
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
                            title={
                                graphDirty ? t("canvas.replayDisabledDirty") :
                                !hasRecording ? t("canvas.replayNoRecording") :
                                t("canvas.replayExecution")
                            }
                        >
                            <VscDebugRestart size={14} />
                        </Button>
                    ) : (
                        <div className="flex items-center">
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={togglePlayPause}
                                className={
                                    isPlaying
                                        ? 'text-amber-400'
                                        : 'text-blue-400'
                                }
                                title={isPlaying ? t("canvas.pauseReplay") : t("canvas.resumeReplay")}
                            >
                                {isPlaying ? <VscDebugPause size={14} /> : <VscPlay size={14} />}
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={stop}
                                className="text-red-400"
                                title={t("canvas.stopReplay")}
                            >
                                <VscDebugStop size={14} />
                            </Button>
                        </div>
                    )}

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    {/* Execute */}
                    <Button
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
                        title={isThisGraphRunning ? t("canvas.executing") : t("canvas.executeCurrentEvent")}
                    >
                        <VscRunAll size={14} />
                    </Button>
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
            {activeGroupId === groupId && variableDropMenu && createPortal(
                <Card
                    className="menu-container fixed z-50 overflow-hidden py-1 shadow-xl"
                    style={{ left: variableDropMenu.x, top: variableDropMenu.y }}
                    onPointerDown={(e) => e.stopPropagation()}
                >
                    <Button
                        type="button"
                        variant="ghost"
                        className="h-auto w-full justify-start rounded-none px-4 py-2 text-sm font-bold"
                        onClick={() => handleVariableDropGet(variableDropMenu)}
                    >
                        <div className="w-2 h-2 rounded-full bg-blue-400" />
                        {t("canvas.getVariable", { name: variableDropMenu.variableName })}
                    </Button>
                    <Separator />
                    <Button
                        type="button"
                        variant="ghost"
                        className="h-auto w-full justify-start rounded-none px-4 py-2 text-sm font-bold"
                        onClick={() => handleVariableDropSet(variableDropMenu)}
                    >
                        <div className="w-2 h-2 rounded-full bg-orange-400" />
                        {t("canvas.setVariable", { name: variableDropMenu.variableName })}
                    </Button>
                </Card>,
                document.body
            )}
        </>
    );
}
