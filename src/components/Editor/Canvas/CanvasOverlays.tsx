import React from "react";
import { useCanvas } from "../Context/CanvasContext";
import { useGestureStore } from "../Store/useGestureStore";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore } from "../Store/useNodeStore";
import { BaseNode } from "../Types/nodes";
import HUD from "./HUD";
import NodePalette from "../Nodes/NodePalette";
import { VscRunAll } from "react-icons/vsc";
import { DEFAULT_VIEWPORT } from "./constants";
import { createNodeFromTemplate } from "../Utils/nodeUtils";

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
        setNodes,
        saveHistory,
        pendingConnection,
        connectPins,
        variables,
        globalVariables,
        functions,
        macros,
        tabs,
        activeTabId,
        activeGroupId,
        groupId,
        executeGraph,
        setCanvas // Needed for internal node centering
    } = useCanvas();

    const gesture = useGestureStore(state => state.gesture);
    const scale = useViewportStore(state => state.viewports[groupId]?.scale || 1);

    const handleNodePaletteSelect = (item: import("../Nodes/NodePalette").PaletteItem) => {
        if (!contextMenu || !canvasRef.current) return;

        // Check if this is an internal node type that should only exist once
        const internalNodeTypes = ['event_on_run', 'function_entry', 'function_return', 'macro_inputs', 'macro_outputs'];
        if (internalNodeTypes.includes(item.type)) {
            // Check if this internal node already exists
            const currentNodes = useNodeStore.getState().getNodes(activeTabId || "");
            const existingNode = currentNodes.find(n => n.type === item.type && n.isInternal);
            if (existingNode) {
                // Move canvas to center on the existing node
                const rect = canvasRef.current.getBoundingClientRect();
                const centerX = rect.width / 2;
                const centerY = rect.height / 2;

                const currentCanvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
                setCanvas({
                    ...currentCanvas,
                    x: centerX - existingNode.position.x * currentCanvas.scale,
                    y: centerY - existingNode.position.y * currentCanvas.scale
                });

                setContextMenu(null);
                setPendingConnection(null);
                return;
            }
        }

        const rect = canvasRef.current.getBoundingClientRect();
        const currentCanvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
        const x = (contextMenu.x - rect.left - currentCanvas.x) / currentCanvas.scale;
        const y = (contextMenu.y - rect.top - currentCanvas.y) / currentCanvas.scale;

        let newNode: BaseNode | null = null;

        if (item.type === 'call_function' || item.type === 'call_macro') {
            const subId = item.overrides?.subGraphId;
            if (subId) {
                const subData = item.type === 'call_function' ? functions[subId] : macros[subId];
                if (subData) {
                    const subName = subData.name;
                    const type = item.type;

                    newNode = new BaseNode(
                        `node_${Date.now()}`,
                        {
                            node_type: type,
                            category: type === 'call_function' ? "Functions" : "Macros",
                            title: subName,
                            inputs: [
                                { id: `exec-in-${Date.now()}`, nodeId: "", name: "In", type: "exec", direction: "input", links: [] },
                                ...(subData.inputs || []).map((p: any) => ({ id: `in-${p.id}-${Date.now()}`, nodeId: "", name: p.name, type: p.type as any, direction: "input", links: [] as string[] }))
                            ],
                            outputs: [
                                { id: `exec-out-${Date.now()}`, nodeId: "", name: "Out", type: "exec", direction: "output", links: [] },
                                ...(subData.outputs || []).map((p: any) => ({ id: `out-${p.id}-${Date.now()}`, nodeId: "", name: p.name, type: p.type as any, direction: "output", links: [] as string[] }))
                            ],
                            ui_style: "default"
                        },
                        { x, y }
                    );
                    newNode.subGraphId = subId;
                    newNode.isInternal = false;
                }
            }
        } else {
            newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, item.type, item.overrides);
        }

        if (newNode) {
            saveHistory();
            setNodes((prev) => [...prev, newNode!]);

            // 如果有待处理的连接，尝试自动连接
            if (pendingConnection) {
                const isInput = pendingConnection.direction === "input";
                const targetDirection = isInput ? "outputs" : "inputs";

                // 寻找新节点中第一个符合类型的引脚
                const pins = targetDirection === "inputs" ? newNode.inputs : newNode.outputs;
                const compatiblePin = pins.find(p => p.type === pendingConnection.type);

                if (compatiblePin) {
                    // 延迟一帧调用 connectPins 确保 nodes 已更新
                    setTimeout(() => {
                        connectPins(pendingConnection.id, compatiblePin.id);
                    }, 0);
                }
            }
        }
        setContextMenu(null);
        setPendingConnection(null);
    };

    return (
        <>
            <HUD />

            {/* ================= FAB (Floating Action Button) for Execution ================= */}
            {tabs.find(t => t.id === activeTabId)?.type === "event" && (
                <div className="absolute top-4 right-4 z-40">
                    <button
                        onClick={() => executeGraph()}
                        className="flex items-center gap-2 px-6 py-2.5 bg-green-600 hover:bg-green-500 text-white rounded-full shadow-lg transition-all active:scale-95 text-xs font-bold ring-4 ring-black/20"
                    >
                        <VscRunAll size={18} />
                        <span>执行</span>
                    </button>
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
                        onSelect={handleNodePaletteSelect}
                        filterPin={pendingConnection}
                        variables={variables}
                        globalVariables={globalVariables}
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
                        onClick={() => {
                            if (!variables[variableDropMenu.variableId] && !globalVariables[variableDropMenu.variableId]) {
                                console.warn("Variable no longer exists.");
                                setVariableDropMenu(null);
                                return;
                            }
                            saveHistory();
                            const newNode = createNodeFromTemplate(
                                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                                scale,
                                "get_variable",
                                {
                                    title: `Get ${variableDropMenu.variableName}`,
                                    variableId: variableDropMenu.variableId,
                                    variableType: variableDropMenu.variableType,
                                    variableName: variableDropMenu.variableName
                                }
                            );
                            if (newNode) {
                                setNodes((prev) => [...prev, newNode]);
                            }
                            setVariableDropMenu(null);
                        }}
                    >
                        <div className="w-2 h-2 rounded-full bg-blue-400" />
                        Get {variableDropMenu.variableName}
                    </div>
                    <div
                        className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2 border-t border-gray-700"
                        onClick={() => {
                            if (!variables[variableDropMenu.variableId] && !globalVariables[variableDropMenu.variableId]) {
                                console.warn("Variable no longer exists.");
                                setVariableDropMenu(null);
                                return;
                            }
                            saveHistory();
                            const newNode = createNodeFromTemplate(
                                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                                scale,
                                "set_variable",
                                {
                                    title: `Set ${variableDropMenu.variableName}`,
                                    variableId: variableDropMenu.variableId,
                                    variableType: variableDropMenu.variableType,
                                    variableName: variableDropMenu.variableName
                                }
                            );
                            if (newNode) {
                                setNodes((prev) => [...prev, newNode]);
                            }
                            setVariableDropMenu(null);
                        }}
                    >
                        <div className="w-2 h-2 rounded-full bg-orange-400" />
                        Set {variableDropMenu.variableName}
                    </div>
                </div>
            )}
        </>
    );
}
