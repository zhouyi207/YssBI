import React from "react";
import type { LayoutTab } from "@/shared/types";
import { useEditorGroup } from "@/features/application/editor";
import { useGestureStore } from "@/features/core/gesture";
import { useExecutionPlayback, useExecutionStore } from "@/features/core/execution";

import { useNodeManagement } from "@/features/application/dataManagement";
import { useCanvasOverlayHandlers } from "@/features/application/editor";
import { HUD } from "./HUD";
import { NodePalette, type PaletteItem } from "../../Layout/NodePalette";
import {
  VscDebugStart,
  VscDebugPause,
  VscDebugStop,
  VscDebugRestart,
  VscPlay,
  VscRunAll,
} from "react-icons/vsc";

export default function CanvasOverlays({
    canvasRef,
    variableDropMenu,
    setVariableDropMenu
}: {
    canvasRef: React.RefObject<HTMLDivElement | null>;
    variableDropMenu: any;
    setVariableDropMenu: (val: any) => void;
}) {
    const {
        contextMenu,
        setContextMenu,
        setPendingConnection,
        pendingConnection,
        variables,
        Variables,
        functions,
        macros,
        tabs,
        activeTabId,
        activeGroupId,
        groupId,
        executeGraph,
        setCanvas,
    } = useEditorGroup();

    const { createNode } = useNodeManagement();

    const gesture = useGestureStore((state) => state.gesture);

    const {
        handleNodePaletteSelect,
        handleVariableDropGet,
        handleVariableDropSet,
    } = useCanvasOverlayHandlers({
        canvasRef,
        groupId,
        activeTabId,
        functions,
        macros,
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
            <HUD />

            {isEventTab && (
                <div className="absolute top-3 right-3 z-40 flex items-center gap-1 bg-[var(--panel-bg)]/80 backdrop-blur-sm border border-[var(--border-color)] rounded-md p-0.5 shadow-lg">
                    {/* Debug — 预留，始终禁用 */}
                    <button
                        disabled
                        className="flex items-center gap-1 px-2.5 py-1.5 rounded text-xs font-medium text-[var(--text-secondary)] opacity-40 cursor-not-allowed"
                        title="调试（即将推出）"
                    >
                        <VscDebugStart size={14} />
                    </button>

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    {/* Replay */}
                    {!playbackActive ? (
                        <button
                            onClick={() => canReplay && togglePlayPause()}
                            disabled={!canReplay}
                            className={`flex items-center gap-1 px-2.5 py-1.5 rounded text-xs font-medium transition-colors ${
                                canReplay
                                    ? 'text-blue-400 hover:bg-blue-500/15 hover:text-blue-300'
                                    : 'text-[var(--text-secondary)] opacity-40 cursor-not-allowed'
                            }`}
                            title={
                                graphDirty ? "图结构已更改，无法回放" :
                                !hasRecording ? "无录制数据" :
                                "回放执行"
                            }
                        >
                            <VscDebugRestart size={14} />
                        </button>
                    ) : (
                        <div className="flex items-center">
                            <button
                                onClick={togglePlayPause}
                                className={`flex items-center gap-1 px-2.5 py-1.5 rounded-l text-xs font-medium transition-colors ${
                                    isPlaying
                                        ? 'text-amber-400 hover:bg-amber-500/15'
                                        : 'text-blue-400 hover:bg-blue-500/15'
                                }`}
                                title={isPlaying ? "暂停回放" : "继续回放"}
                            >
                                {isPlaying ? <VscDebugPause size={14} /> : <VscPlay size={14} />}
                            </button>
                            <button
                                onClick={stop}
                                className="flex items-center px-2 py-1.5 rounded-r text-xs font-medium text-red-400 hover:bg-red-500/15 transition-colors"
                                title="停止回放"
                            >
                                <VscDebugStop size={14} />
                            </button>
                        </div>
                    )}

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    {/* Execute */}
                    <button
                        onClick={() => !isThisGraphRunning && executeGraph(tabId)}
                        disabled={isThisGraphRunning}
                        className={`flex items-center gap-1 px-2.5 py-1.5 rounded text-xs font-medium transition-colors ${
                            isThisGraphRunning
                                ? 'text-green-400 opacity-60 cursor-not-allowed'
                                : 'text-green-400 hover:bg-green-500/15 hover:text-green-300'
                        }`}
                        title={isThisGraphRunning ? "执行中…" : "执行当前 Event"}
                    >
                        <VscRunAll size={14} />
                    </button>
                </div>
            )}

            {/* ================= Selection Box ================= */}
            {gesture?.type === 'select' && gesture?.groupId === groupId && canvasRef.current && (
                <div
                    className="absolute pointer-events-none z-50 border-2 border-dashed border-[var(--accent-color)] bg-[var(--selection-region)]/15"
                    style={{
                        left:
                            Math.min(gesture.startX, gesture.currentX) -
                            canvasRef.current.getBoundingClientRect().left,
                        top:
                            Math.min(gesture.startY, gesture.currentY) -
                            canvasRef.current.getBoundingClientRect().top,
                        width: Math.abs(gesture.startX - gesture.currentX),
                        height: Math.abs(gesture.startY - gesture.currentY),
                    }}
                />
            )}

            {/* ================= Node Palette ================= */}
            {activeGroupId === groupId && contextMenu?.visible && (
                <div className="menu-container">
                    <NodePalette
                        x={contextMenu.x}
                        y={contextMenu.y}
                        onSelect={onPaletteSelect}
                        filterPin={pendingConnection}
                        variables={variables}
                        Variables={Variables}
                        functions={functions}
                        macros={macros}
                    />
                </div>
            )}

            {/* ================= Variable Drop Menu ================= */}
            {activeGroupId === groupId && variableDropMenu && (
                <div
                    className="fixed z-50 bg-gray-800 text-white rounded shadow-lg overflow-hidden border border-gray-700 py-1 menu-container"
                    style={{ left: variableDropMenu.x, top: variableDropMenu.y }}
                    onPointerDown={(e) => e.stopPropagation()}
                >
                    <div
                        className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2"
                        onClick={() => handleVariableDropGet(variableDropMenu)}
                    >
                        <div className="w-2 h-2 rounded-full bg-blue-400" />
                        Get {variableDropMenu.variableName}
                    </div>
                    <div
                        className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2 border-t border-gray-700"
                        onClick={() => handleVariableDropSet(variableDropMenu)}
                    >
                        <div className="w-2 h-2 rounded-full bg-orange-400" />
                        Set {variableDropMenu.variableName}
                    </div>
                </div>
            )}
        </>
    );
}
