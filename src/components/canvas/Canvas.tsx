import { useRef, useState, useEffect, useCallback, useMemo, useLayoutEffect } from "react";
import { Node } from "../node/Node";
import { BaseNode, Pin } from "../node/models";
import { useDrag } from "../drag/DragContext";
import { useCanvas } from "./CanvasContext";
import { createNodeFromTemplate } from "../node/util";
import HUD from "./HUD";
import NodePalette from "./NodePalette";
import { Edge } from "../Edge";

/* ================= Canvas ================= */

export default function InfiniteCanvas() {
  const {
    canvas,
    onCanvasWheel,
    onCanvasPointerDown,
    onPinPointerDown,
    selection,
    gesture,
    setGesture,
    contextMenu,
    setContextMenu
  } = useCanvas();
  const { drag } = useDrag();

  const [nodes, setNodes] = useState<BaseNode[]>([]);
  const [selectedNodeIds, setSelectedNodeIds] = useState<Set<string>>(new Set());
  const selectedNodeIdsRef = useRef(selectedNodeIds);

  useEffect(() => {
    selectedNodeIdsRef.current = selectedNodeIds;
  }, [selectedNodeIds]);

  const [variableDropMenu, setVariableDropMenu] = useState<{
    x: number;
    y: number;
    worldX: number;
    worldY: number;
    varType: string;
  } | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  const prevDragRef = useRef(drag);
  const prevSelectionRef = useRef(selection);

  const pinIndex = useMemo(() => {
    const map = new Map<string, Pin>();
    nodes.forEach((node) => {
      node.inputs.forEach((pin) => map.set(pin.id, pin));
      node.outputs.forEach((pin) => map.set(pin.id, pin));
    });
    return map;
  }, [nodes]);

  const pinNodeIdIndex = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach((node) => {
      node.inputs.forEach((pin) => map.set(pin.id, node.id));
      node.outputs.forEach((pin) => map.set(pin.id, node.id));
    });
    return map;
  }, [nodes]);

  const pinIndexRef = useRef(pinIndex);
  useEffect(() => {
    pinIndexRef.current = pinIndex;
  }, [pinIndex]);

  const pinNodeIdIndexRef = useRef(pinNodeIdIndex);
  useEffect(() => {
    pinNodeIdIndexRef.current = pinNodeIdIndex;
  }, [pinNodeIdIndex]);

  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});

  // 测量 Pin 相对于节点的偏移量 (仅在节点或 Pin 数量变化时运行)
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const nextOffsets: Record<string, { x: number; y: number }> = {};

    nodes.forEach(node => {
      const nodeEl = root.querySelector(`[id="${node.id}"]`);
      if (!nodeEl) return;
      const nodeRect = nodeEl.getBoundingClientRect();

      const pins = nodeEl.querySelectorAll<HTMLElement>("[data-pin-id]");
      pins.forEach(pinEl => {
        const pinId = pinEl.dataset.pinId;
        if (!pinId) return;
        const circleEl = pinEl.querySelector(".pin-circle");
        const targetEl = circleEl || pinEl;
        const rect = targetEl.getBoundingClientRect();

        // 计算相对于节点左上角的偏移，并还原缩放
        nextOffsets[pinId] = {
          x: (rect.left + rect.width / 2 - nodeRect.left) / canvas.scale,
          y: (rect.top + rect.height / 2 - nodeRect.top) / canvas.scale,
        };
      });
    });

    setPinOffsets(prev => {
      // 简单深度比较，避免无谓更新
      if (Object.keys(prev).length === Object.keys(nextOffsets).length) {
        let same = true;
        for (const key in nextOffsets) {
          if (!prev[key] || prev[key].x !== nextOffsets[key].x || prev[key].y !== nextOffsets[key].y) {
            same = false;
            break;
          }
        }
        if (same) return prev;
      }
      return nextOffsets;
    });
  }, [nodes.length, nodes.map(n => n.inputs.length + n.outputs.length).join(",")]); // 仅在节点数或针脚数变化时重新测量

  const nodeMap = useMemo(() => {
    const map = new Map<string, BaseNode>();
    nodes.forEach(n => map.set(n.id, n));
    return map;
  }, [nodes]);

  // 获取 Pin 的世界坐标 (Node位置 + 偏移)
  const getPinWorldPos = useCallback((pinId: string) => {
    const nodeId = pinNodeIdIndexRef.current.get(pinId);
    const node = nodeMap.get(nodeId || "");
    const offset = pinOffsets[pinId];
    if (!node || !offset) return { x: 0, y: 0 };
    return {
      x: node.position.x + offset.x,
      y: node.position.y + offset.y
    };
  }, [nodeMap, pinOffsets]);

  const getCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
    const root = ref.current;
    if (!root) return { x: 0, y: 0 };
    const rect = root.getBoundingClientRect();
    // 转换为 Canvas World 坐标
    return {
      x: (clientX - rect.left - canvas.x) / canvas.scale,
      y: (clientY - rect.top - canvas.y) / canvas.scale
    };
  }, [canvas.x, canvas.y, canvas.scale]);

  const isCompatiblePins = (a: Pin, b: Pin) => {
    return a.direction !== b.direction && a.type === b.type;
  };

  const isSingleLinkPin = (pin: Pin) => {
    if (pin.type === "exec") return true;
    return pin.direction === "input";
  };

  const updatePinLink = (node: BaseNode, pinId: string, otherId: string) => {
    const pin = [...node.inputs, ...node.outputs].find((p) => p.id === pinId);
    if (!pin) return false;

    if (isSingleLinkPin(pin)) {
      if (pin.links.length === 1 && pin.links[0] === otherId) return false;
      pin.links = [otherId];
    } else {
      if (pin.links.includes(otherId)) return false;
      pin.links = [...pin.links, otherId];
    }
    return true;
  };

  const removePinLink = (node: BaseNode, pinId: string, otherId: string) => {
    const pin = [...node.inputs, ...node.outputs].find((p) => p.id === pinId);
    if (!pin || !pin.links.includes(otherId)) return false;
    pin.links = pin.links.filter(id => id !== otherId);
    return true;
  };

  const connectPins = useCallback((nodes: BaseNode[], pinAId: string, pinBId: string) => {
    const pinA = pinIndexRef.current.get(pinAId);
    const pinB = pinIndexRef.current.get(pinBId);
    if (!pinA || !pinB || !isCompatiblePins(pinA, pinB)) return nodes;

    const outputPin = pinA.direction === "output" ? pinA : pinB;
    const inputPin = pinA.direction === "input" ? pinA : pinB;
    const outputNodeId = pinNodeIdIndexRef.current.get(outputPin.id);
    const inputNodeId = pinNodeIdIndexRef.current.get(inputPin.id);
    if (!outputNodeId || !inputNodeId) return nodes;

    // 记录需要清理的旧连接
    const toCleanup: { pinId: string; oldPeerId: string }[] = [];
    if (isSingleLinkPin(outputPin) && outputPin.links.length > 0) {
      toCleanup.push({ pinId: outputPin.id, oldPeerId: outputPin.links[0] });
    }
    if (isSingleLinkPin(inputPin) && inputPin.links.length > 0) {
      toCleanup.push({ pinId: inputPin.id, oldPeerId: inputPin.links[0] });
    }

    return nodes.map((node) => {
      let nodeChanged = false;

      // 检查是否是需要清理旧连接的节点
      toCleanup.forEach(({ pinId, oldPeerId }) => {
        if (node.id === pinNodeIdIndexRef.current.get(oldPeerId)) {
          const newNode = nodeChanged ? (node as any) : node.clone();
          if (removePinLink(newNode, oldPeerId, pinId)) {
            nodeChanged = true;
            node = newNode;
          }
        }
      });

      // 检查是否是需要建立新连接的节点
      if (node.id === outputNodeId || node.id === inputNodeId) {
        const newNode = nodeChanged ? (node as any) : node.clone();
        const a = updatePinLink(newNode, outputPin.id, inputPin.id);
        const b = updatePinLink(newNode, inputPin.id, outputPin.id);
        if (a || b) {
          nodeChanged = true;
          node = newNode;
        }
      }

      return node;
    });
  }, []);

  const lastConnectRef = useRef<{ startPin: Pin } | null>(null);

  useEffect(() => {
    if (gesture?.type === "connect") {
      lastConnectRef.current = { startPin: gesture.startPin };

      // 需求 4: Ctrl + 移动引脚连接 (重连逻辑)
      if (gesture.isReconnect && gesture.startPin.links.length > 0) {
        const sourcePin = gesture.startPin;
        const peerId = sourcePin.links[sourcePin.links.length - 1]; // 取最后一个连接
        const peerPin = pinIndexRef.current.get(peerId);

        if (peerPin) {
          // 1. 断开原有的连接
          const sourceNodeId = pinNodeIdIndexRef.current.get(sourcePin.id);
          const peerNodeId = pinNodeIdIndexRef.current.get(peerPin.id);

          setNodes(prev => prev.map(n => {
            if (n.id === sourceNodeId || n.id === peerNodeId) {
              const newNode = n.clone();
              removePinLink(newNode, n.id === sourceNodeId ? sourcePin.id : peerPin.id, n.id === sourceNodeId ? peerPin.id : sourcePin.id);
              return newNode;
            }
            return n;
          }));

          // 2. 将连接起点切换为对端 Pin，实现“从对端拉出线”的效果
          setGesture({
            ...gesture,
            startPin: peerPin,
            isReconnect: false // 已经处理过重连，转为普通连接
          });
        }
      }
    } else if (gesture?.type === "disconnect") {
      // 需求 5: Alt + 点击断开
      const targetPin = gesture.pin;
      const peerIds = [...targetPin.links];
      const targetNodeId = pinNodeIdIndexRef.current.get(targetPin.id);

      setNodes(prev => prev.map(n => {
        let changed = false;
        const newNode = n.id === targetNodeId || peerIds.some(id => pinNodeIdIndexRef.current.get(id) === n.id) ? n.clone() : n;

        if (n.id === targetNodeId) {
          const p = [...newNode.inputs, ...newNode.outputs].find(p => p.id === targetPin.id);
          if (p && p.links.length > 0) {
            p.links = [];
            changed = true;
          }
        }

        peerIds.forEach(peerId => {
          if (pinNodeIdIndexRef.current.get(peerId) === n.id) {
            if (removePinLink(newNode, peerId, targetPin.id)) {
              changed = true;
            }
          }
        });

        return changed ? newNode : n;
      }));

      setGesture(null);
    }
  }, [gesture, setGesture]); // 添加 setGesture 依赖项

  // 处理连接逻辑
  useEffect(() => {
    const handlePointerUp = (e: PointerEvent) => {
      const lastConnect = lastConnectRef.current;
      if (!lastConnect) return;

      // 清理，防止重复触发 (除非新的 gesture 再次设置它)
      lastConnectRef.current = null;

      // 使用 elementsFromPoint 以穿透可能存在的遮挡物
      const elements = document.elementsFromPoint(e.clientX, e.clientY);
      const pinEl = elements.find(el => el.closest("[data-pin-id]"))?.closest("[data-pin-id]");

      const targetPinId = pinEl?.getAttribute("data-pin-id");
      const sourcePin = lastConnect.startPin;

      if (!targetPinId || targetPinId === sourcePin.id) return;

      const source = pinIndexRef.current.get(sourcePin.id);
      const target = pinIndexRef.current.get(targetPinId);
      if (!source || !target) return;
      if (!isCompatiblePins(source, target)) return;

      const outputPin = source.direction === "output" ? source : target;
      const inputPin = source.direction === "input" ? source : target;
      const outputNodeId = pinNodeIdIndexRef.current.get(outputPin.id);
      const inputNodeId = pinNodeIdIndexRef.current.get(inputPin.id);
      if (!outputNodeId || !inputNodeId) return;

      setNodes((prev) => {
        // 记录需要清理的旧连接
        const toCleanup: { pinId: string; oldPeerId: string }[] = [];
        if (isSingleLinkPin(outputPin) && outputPin.links.length > 0) {
          toCleanup.push({ pinId: outputPin.id, oldPeerId: outputPin.links[0] });
        }
        if (isSingleLinkPin(inputPin) && inputPin.links.length > 0) {
          toCleanup.push({ pinId: inputPin.id, oldPeerId: inputPin.links[0] });
        }

        let changed = false;
        const next = prev.map((node) => {
          let nodeChanged = false;

          // 检查是否是需要清理旧连接的节点
          toCleanup.forEach(({ pinId, oldPeerId }) => {
            // 如果节点包含 oldPeerId，需要移除对 pinId 的引用
            if (node.id === pinNodeIdIndexRef.current.get(oldPeerId)) {
              const newNode = nodeChanged ? (node as any) : node.clone();
              if (removePinLink(newNode, oldPeerId, pinId)) {
                nodeChanged = true;
                node = newNode;
              }
            }
          });

          // 检查是否是需要建立新连接的节点
          if (node.id === outputNodeId || node.id === inputNodeId) {
            const newNode = nodeChanged ? (node as any) : node.clone();
            const a = updatePinLink(newNode, outputPin.id, inputPin.id);
            const b = updatePinLink(newNode, inputPin.id, outputPin.id);
            if (a || b) {
              nodeChanged = true;
              node = newNode;
            }
          }

          if (nodeChanged) changed = true;
          return node;
        });
        return changed ? next : prev;
      });
    };

    window.addEventListener("pointerup", handlePointerUp, { capture: true });
    return () => window.removeEventListener("pointerup", handlePointerUp, { capture: true });
  }, []); // 仅在挂载时添加一次监听器，通过 Ref 获取最新状态

  const handleNodePaletteSelect = (tpl: { type: string }) => {
    if (!contextMenu || !ref.current) return;

    const rect = ref.current.getBoundingClientRect();
    const x = (contextMenu.x - rect.left - canvas.x) / canvas.scale;
    const y = (contextMenu.y - rect.top - canvas.y) / canvas.scale;

    const newNode = createNodeFromTemplate({ x, y }, canvas.scale, tpl.type);
    if (newNode) {
      setNodes((prev) => [...prev, newNode]);
    }
    setContextMenu(null);
  };

  // 1. 自动隐藏菜单逻辑
  useEffect(() => {
    const handleClickOutside = (e: PointerEvent) => {
      const target = e.target as HTMLElement;
      // 检查点击是否在菜单容器之外
      const isInsideMenu = target.closest(".menu-container");
      if (!isInsideMenu) {
        if (contextMenu?.visible) setContextMenu(null);
        if (variableDropMenu) setVariableDropMenu(null);
      }
    };

    window.addEventListener("pointerdown", handleClickOutside, true); // 使用捕获阶段
    return () => window.removeEventListener("pointerdown", handleClickOutside, true);
  }, [contextMenu, variableDropMenu, setContextMenu]);

  useEffect(() => {
    if (selection && ref.current) {
      const rect = ref.current.getBoundingClientRect();
      const box = {
        x1: (Math.min(selection.startX, selection.currentX) - rect.left - canvas.x) / canvas.scale,
        y1: (Math.min(selection.startY, selection.currentY) - rect.top - canvas.y) / canvas.scale,
        x2: (Math.max(selection.startX, selection.currentX) - rect.left - canvas.x) / canvas.scale,
        y2: (Math.max(selection.startY, selection.currentY) - rect.top - canvas.y) / canvas.scale,
      };

      const newSelected = new Set<string>();
      let changed = false;

      setNodes(prev => {
        const nextNodes = prev.map(n => {
          const nodeWidth = n.noHeader ? 80 : 150;
          const nodeHeight = n.noHeader ? 60 : 100;
          const isIntersecting =
            n.position.x < box.x2 &&
            n.position.x + nodeWidth > box.x1 &&
            n.position.y < box.y2 &&
            n.position.y + nodeHeight > box.y1;

          if (isIntersecting) newSelected.add(n.id);

          if (n.selected !== isIntersecting) {
            changed = true;
            const newNode = n.clone();
            newNode.selected = isIntersecting;
            return newNode;
          }
          return n;
        });

        if (changed) {
          setSelectedNodeIds(newSelected);
          return nextNodes;
        }
        return prev;
      });
    } else if (prevSelectionRef.current && !selection) {
      const s = prevSelectionRef.current;
      if (Math.abs(s.startX - s.currentX) < 5 && Math.abs(s.startY - s.currentY) < 5) {
        setNodes(prev => {
          let hasSelected = false;
          const nextNodes = prev.map(n => {
            if (n.selected) {
              hasSelected = true;
              const newNode = n.clone();
              newNode.selected = false;
              return newNode;
            }
            return n;
          });
          if (hasSelected) {
            setSelectedNodeIds(new Set());
            return nextNodes;
          }
          return prev;
        });
      }
    }
    prevSelectionRef.current = selection;
  }, [selection, canvas.x, canvas.y, canvas.scale]);

  const edges = useMemo(() => {
    const result: React.ReactNode[] = [];
    nodes.forEach((node) => {
      node.outputs.forEach((pin) => {
        pin.links.forEach((targetId) => {
          const start = getPinWorldPos(pin.id);
          const end = getPinWorldPos(targetId);
          if (start.x === 0 && start.y === 0) return; // 尚未测量
          result.push(
            <Edge
              key={`${pin.id}-${targetId}`}
              x1={start.x}
              y1={start.y}
              x2={end.x}
              y2={end.y}
              color={pin.ui?.color ?? (pin.type === "exec" ? "#ffffff" : "#3b82f6")}
            />
          );
        });
      });
    });
    return result;
  }, [nodes, pinOffsets, getPinWorldPos]);

  // 2. 多节点拖拽回调
  const handleNodeDrag = useCallback((id: string, dx: number, dy: number) => {
    const currentSelected = selectedNodeIdsRef.current;
    setNodes((prev) =>
      prev.map((node) => {
        if (currentSelected.has(id)) {
          if (currentSelected.has(node.id)) {
            return node.cloneWithPosition({
              x: node.position.x + dx,
              y: node.position.y + dy,
            });
          }
        } else if (node.id === id) {
          return node.cloneWithPosition({
            x: node.position.x + dx,
            y: node.position.y + dy,
          });
        }
        return node;
      })
    );
  }, []);

  // 3. 动态添加输入
  const handleNodeAddInput = useCallback((id: string) => {
    setNodes((prev) =>
      prev.map((node) => {
        if (node.id === id) {
          const newNode = node.clone();
          const newIndex = newNode.inputs.length;
          newNode.addInput({
            id: `${id}-input-${newIndex}-${Date.now()}`,
            nodeId: id,
            name: String.fromCharCode(65 + newIndex),
            type: "int", // 修改这里：PinType 中已改为 int
            direction: "input",
            links: [],
          });
          return newNode;
        }
        return node;
      })
    );
  }, []);

  const handlePinClick = useCallback((pinId: string, _direction: "input" | "output") => {
    console.log(`Pin clicked: ${pinId}`);
  }, []);

  useEffect(() => {
    // 从「有拖拽」→「无拖拽」 = drop
    if (prevDragRef.current && !drag) {
      const last = prevDragRef.current;

      if (last.type === "node-template") {
        handleDropTemplate(last, {
          altKey: (window as any)._lastAltKey || false,
          ctrlKey: (window as any)._lastCtrlKey || false,
        } as any);
      }
    }

    prevDragRef.current = drag;
  }, [drag]);

  // 全局记录修饰键状态
  useEffect(() => {
    const handleKeys = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;
    };
    window.addEventListener("keydown", handleKeys);
    window.addEventListener("keyup", handleKeys);
    return () => {
      window.removeEventListener("keydown", handleKeys);
      window.removeEventListener("keyup", handleKeys);
    };
  }, []);

  /* ===== Data Drag ===== */
  function handleDropTemplate(dragState: any, event: MouseEvent | PointerEvent) {
    const el = ref.current;
    if (!el) return;

    const rect = el.getBoundingClientRect();

    // 边界检查：确保释放点在画布范围内
    const isInside =
      dragState.x >= rect.left &&
      dragState.x <= rect.right &&
      dragState.y >= rect.top &&
      dragState.y <= rect.bottom;

    if (!isInside) return;

    const screenX = dragState.x - rect.left;
    const screenY = dragState.y - rect.top;

    const x = (screenX - canvas.x) / canvas.scale;
    const y = (screenY - canvas.y) / canvas.scale;

    // 检查是否落在了 pin 上 (需求 3: 拖动变量到赋值框)
    const elements = document.elementsFromPoint(dragState.x, dragState.y);
    const pinEl = elements.find(e => e.closest("[data-pin-id]"))?.closest("[data-pin-id]");
    const targetPinId = pinEl?.getAttribute("data-pin-id");

    // 如果是变量
    if (dragState.template.category === "Variable") {
      let spawnType: "get_variable" | "set_variable" | null = null;

      if (event.altKey) spawnType = "set_variable";
      else if (event.ctrlKey) spawnType = "get_variable";

      if (spawnType) {
        const newNode = createNodeFromTemplate({ x, y }, canvas.scale, spawnType);
        if (newNode) {
          setNodes((prev) => [...prev, newNode]);

          if (targetPinId && spawnType === "get_variable") {
            const targetPin = pinIndexRef.current.get(targetPinId);
            if (targetPin && targetPin.direction === "input") {
              const outputPin = newNode.outputs[0];
              if (outputPin) {
                setNodes(prev => connectPins(prev, outputPin.id, targetPin.id));
              }
            }
          }
        }
        return;
      }

      if (targetPinId) {
        const targetPin = pinIndexRef.current.get(targetPinId);
        if (targetPin && targetPin.direction === "input") {
          const newNode = createNodeFromTemplate({ x, y }, canvas.scale, "get_variable");
          if (newNode) {
            setNodes(prev => {
              const next = [...prev, newNode];
              const outputPin = newNode.outputs[0];
              if (outputPin) {
                return connectPins(next, outputPin.id, targetPin.id);
              }
              return next;
            });
            return;
          }
        }
      }

      setVariableDropMenu({
        x: dragState.x,
        y: dragState.y,
        worldX: x,
        worldY: y,
        varType: dragState.template.type,
      });
      return;
    }

    const newNode = createNodeFromTemplate(
      { x, y },
      canvas.scale,
      dragState.template.type
    );
    if (newNode) {
      setNodes((prev) => [...prev, newNode]);
    }
  }

  const GRID = 40;

  return (
    <div
      ref={ref}
      className="relative w-full h-full overflow-hidden bg-gray-900 select-none"
      onWheel={onCanvasWheel}
    >
      {/* ================= Grid ================= */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          backgroundImage: `
            linear-gradient(#333 1px, transparent 1px),
            linear-gradient(90deg, #333 1px, transparent 1px)
          `,
          backgroundSize: `${GRID * canvas.scale}px ${GRID * canvas.scale}px`,
          backgroundPosition: `${canvas.x}px ${canvas.y}px`,
        }}
      />

      {/* ================= World ================= */}
      <div className="absolute inset-0" onPointerDown={onCanvasPointerDown}>
        <div
          style={{
            transform: `translate(${canvas.x}px, ${canvas.y}px) scale(${canvas.scale})`,
            transformOrigin: "0 0",
          }}
        >

          {/* Connections in World Space */}
          <svg className="absolute inset-0 overflow-visible pointer-events-none">
            {edges}
            {gesture?.type === "connect" && (
              <Edge
                x1={getPinWorldPos(gesture.startPin.id).x}
                y1={getPinWorldPos(gesture.startPin.id).y}
                x2={getCanvasLocalPoint(gesture.currentX, gesture.currentY).x}
                y2={getCanvasLocalPoint(gesture.currentX, gesture.currentY).y}
                color={gesture.startPin.ui?.color ?? (gesture.startPin.type === "exec" ? "#ffffff" : "#3b82f6")}
                startIsInput={gesture.startPin.direction === "input"}
              />
            )}
          </svg>

          {nodes.map((node) => (
            <Node
              key={node.id}
              node={node}
              scale={canvas.scale}
              onDrag={handleNodeDrag}
              onAddInput={handleNodeAddInput}
              onPinClick={handlePinClick}
              onPinPointerDown={onPinPointerDown}
            />
          ))}
        </div>
      </div>
      {/* ================= HUD ================= */}
      <HUD />

      {/* ================= Selection Box ================= */}
      {selection && ref.current && (
        <div
          className="absolute border border-blue-500 bg-blue-500/20 pointer-events-none z-50"
          style={{
            left:
              Math.min(selection.startX, selection.currentX) -
              ref.current.getBoundingClientRect().left,
            top:
              Math.min(selection.startY, selection.currentY) -
              ref.current.getBoundingClientRect().top,
            width: Math.abs(selection.startX - selection.currentX),
            height: Math.abs(selection.startY - selection.currentY),
          }}
        />
      )}

      {/* ================= Node Palette ================= */}
      {contextMenu?.visible && (
        <NodePalette
          x={contextMenu.x}
          y={contextMenu.y}
          onSelect={handleNodePaletteSelect}
        />
      )}

      {/* ================= Variable Drop Menu ================= */}
      {variableDropMenu && (
        <div
          className="fixed z-50 bg-gray-800 text-white rounded shadow-lg overflow-hidden border border-gray-700 py-1 menu-container"
          style={{ left: variableDropMenu.x, top: variableDropMenu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <div
            className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2"
            onClick={() => {
              const newNode = createNodeFromTemplate(
                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                canvas.scale,
                "get_variable"
              );
              if (newNode) setNodes((prev) => [...prev, newNode]);
              setVariableDropMenu(null);
            }}
          >
            <div className="w-2 h-2 rounded-full bg-blue-400" />
            Get Variable
          </div>
          <div
            className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2 border-t border-gray-700"
            onClick={() => {
              const newNode = createNodeFromTemplate(
                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                canvas.scale,
                "set_variable"
              );
              if (newNode) setNodes((prev) => [...prev, newNode]);
              setVariableDropMenu(null);
            }}
          >
            <div className="w-2 h-2 rounded-full bg-orange-400" />
            Set Variable
          </div>
        </div>
      )}
    </div>
  );
}
