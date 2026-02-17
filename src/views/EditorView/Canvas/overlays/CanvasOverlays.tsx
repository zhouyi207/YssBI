import React, { useState } from "react";
import type { LayoutTab } from "@/shared/types";
import { useEditorGroup } from "@/features/application/editor";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from "@/features/core/viewport";
import { useNodeManagement } from "@/features/application/dataManagement";
import { useCanvasOverlayHandlers } from "@/features/application/editor";
import { HUD } from "./HUD";
import { NodePalette, type PaletteItem } from "../../Layout/NodePalette";
import { VscRunAll, VscChevronDown } from "react-icons/vsc";

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
        executeAllEvents,
        setCanvas,
    } = useEditorGroup();

    const { createNode } = useNodeManagement();
    const [showExecuteMenu, setShowExecuteMenu] = useState(false);

    const gesture = useGestureStore((state) => state.gesture);
    const scale = useViewportStore((state) => state.viewports[groupId]?.scale || 1);

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
        scale,
        variables,
        Variables,
        setContextMenu,
        setPendingConnection,
        setVariableDropMenu,
        createNode,
        setCanvas,
    });

    const onPaletteSelect = (item: PaletteItem) =>
        contextMenu && handleNodePaletteSelect(item, contextMenu);

    return (
        <>
            <HUD />

            {/* ================= FAB (Floating Action Button) for Execution ================= */}
            {tabs.find((t: LayoutTab) => t.id === activeTabId)?.type === "event" && (
                <div className="absolute top-4 right-4 z-40">
                    <div className="relative">
                        <div className="flex items-center gap-1">
                            {/* 主执行按钮 */}
                            <button
                                onClick={() => executeGraph()}
                                className="flex items-center gap-2 px-6 py-2.5 bg-green-600 hover:bg-green-500 text-white rounded-l-full shadow-lg transition-all active:scale-95 text-xs font-bold ring-4 ring-black/20"
                            >
                                <VscRunAll size={18} />
                                <span>执行当前</span>
                            </button>
                            {/* 下拉按钮 */}
                            <button
                                onClick={() => setShowExecuteMenu(!showExecuteMenu)}
                                className="flex items-center px-2 py-2.5 bg-green-600 hover:bg-green-500 text-white rounded-r-full shadow-lg transition-all active:scale-95 text-xs font-bold ring-4 ring-black/20 border-l border-green-700"
                            >
                                <VscChevronDown size={14} />
                            </button>
                        </div>

                        {/* 下拉菜单 */}
                        {showExecuteMenu && (
                            <>
                                {/* 点击遮罩关闭菜单 */}
                                <div
                                    className="fixed inset-0 z-40"
                                    onClick={() => setShowExecuteMenu(false)}
                                />
                                <div className="absolute top-full right-0 mt-2 w-48 bg-[var(--panel-bg)] border border-[var(--border-color)] rounded-lg shadow-xl z-50 overflow-hidden">
                                    <button
                                        onClick={() => {
                                            executeGraph();
                                            setShowExecuteMenu(false);
                                        }}
                                        className="w-full px-4 py-2.5 text-left text-sm text-[var(--text-primary)] hover:bg-[var(--hover-bg)] flex items-center gap-2"
                                    >
                                        <VscRunAll size={16} />
                                        <span>执行当前 Event</span>
                                    </button>
                                    <div className="h-px bg-[var(--border-color)]" />
                                    <button
                                        onClick={() => {
                                            executeAllEvents();
                                            setShowExecuteMenu(false);
                                        }}
                                        className="w-full px-4 py-2.5 text-left text-sm text-[var(--text-primary)] hover:bg-[var(--hover-bg)] flex items-center gap-2"
                                    >
                                        <VscRunAll size={16} />
                                        <span>执行所有 Events</span>
                                    </button>
                                </div>
                            </>
                        )}
                    </div>
                </div>
            )}

            {/* ================= Selection Box ================= */}
            {gesture?.type === 'select' && canvasRef.current && (
                <div
                    className="absolute border border-[var(--accent-color)] bg-[var(--selection-region)] pointer-events-none z-50"
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
