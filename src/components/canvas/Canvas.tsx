import { useRef, useState, useEffect, useCallback, useMemo, useLayoutEffect } from "react";
import { Node } from "../node/Node";
import { BaseNode, Pin } from "../node/models";
import { useDrag } from "../drag/DragContext";
import { useCanvas } from "./CanvasContext";
import { createNodeFromTemplate } from "../node/util";
import { NODE_REGISTRY } from "../node/registry";
import HUD from "./HUD";
import NodePalette from "./NodePalette";
import { drawEdge } from "../Edge";

/* ================= Canvas ================= */

export default function InfiniteCanvas() {
  const {
    canvas,
    setCanvas,
    nodes,
    setNodes,
    onCanvasPointerDown,
    onPinPointerDown,
    selection,
    gesture,
    setGesture,
    contextMenu,
    setContextMenu,
    setSelectedVariableId,
    variables
  } = useCanvas();
  const { drag } = useDrag();

  // 优化：使用 Ref 记录最新的 canvas 状态，避免 useEffect 频繁卸载/挂载监听器
  const canvasRef = useRef(canvas);
  useEffect(() => {
    canvasRef.current = canvas;
  }, [canvas]);

  // 使用原生事件监听器以更好地控制 preventDefault 和 stopPropagation
  useEffect(() => {
    const canvasEl = ref.current;
    if (!canvasEl) return;

    const handleWheel = (e: WheelEvent) => {
      // 检查点击来源：如果是 UI 组件，则完全忽略
      const target = e.target as HTMLElement;
      if (
        target.closest(".menubar-container") || 
        target.closest(".sidebar-container") ||
        target.closest(".menu-container") ||
        target.closest(".hud-container")
      ) {
        return;
      }

      // 如果鼠标不在画布范围内，也不处理
      const rect = canvasEl.getBoundingClientRect();
      if (
        e.clientX < rect.left ||
        e.clientX > rect.right ||
        e.clientY < rect.top ||
        e.clientY > rect.bottom
      ) {
        return;
      }

      // 阻止浏览器默认的缩放或滚动行为
      e.preventDefault();
      
      // 直接在此处执行缩放逻辑，使用 Ref 保证性能和稳定性
      const factor = 0.001;
      const currentCanvas = canvasRef.current;
      const nextScale = Math.min(Math.max(currentCanvas.scale * (1 - e.deltaY * factor), 0.2), 4);

      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const worldX = (mouseX - currentCanvas.x) / currentCanvas.scale;
      const worldY = (mouseY - currentCanvas.y) / currentCanvas.scale;

      setCanvas({
        scale: nextScale,
        x: mouseX - worldX * nextScale,
        y: mouseY - worldY * nextScale,
      });
    };

    window.addEventListener("wheel", handleWheel, { passive: false, capture: true });
    return () => window.removeEventListener("wheel", handleWheel, { capture: true });
  }, [setCanvas]); // 仅依赖 setCanvas

  const edgeCanvasRef = useRef<HTMLCanvasElement>(null);

  const selectedNodeIds = useMemo(() => {
    const set = new Set<string>();
    nodes.forEach(n => {
      if (n.selected) set.add(n.id);
    });
    return set;
  }, [nodes]);

  const selectedNodeIdsRef = useRef(selectedNodeIds);
  useEffect(() => {
    selectedNodeIdsRef.current = selectedNodeIds;
  }, [selectedNodeIds]);

  const [variableDropMenu, setVariableDropMenu] = useState<{
    x: number;
    y: number;
    worldX: number;
    worldY: number;
    variableId: string;
    variableName: string;
    variableType: string;
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

  // 绘制连接线的核心逻辑 (GPU 加速)
  const drawAllEdges = useCallback(() => {
    const canvasEl = edgeCanvasRef.current;
    if (!canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    // 清除画布
    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

    // 设置变换矩阵 (同步画布的平移和缩放)
    ctx.save();
    ctx.translate(canvas.x, canvas.y);
    ctx.scale(canvas.scale, canvas.scale);

    // 绘制已有连接
    nodes.forEach((node) => {
      node.outputs.forEach((pin) => {
        pin.links.forEach((targetId) => {
          const start = getPinWorldPos(pin.id);
          const end = getPinWorldPos(targetId);
          if (start.x === 0 && start.y === 0) return;
          
          drawEdge(
            ctx,
            start.x, start.y,
            end.x, end.y,
            pin.ui?.color ?? (pin.type === "exec" ? "#ffffff" : "#3b82f6"),
            2 / canvas.scale // 保持视觉粗细一致
          );
        });
      });
    });

    // 绘制当前正在拖拽的连接线
    if (gesture?.type === "connect") {
      const start = getPinWorldPos(gesture.startPin.id);
      const end = getCanvasLocalPoint(gesture.currentX, gesture.currentY);
      drawEdge(
        ctx,
        start.x, start.y,
        end.x, end.y,
        gesture.startPin.ui?.color ?? (gesture.startPin.type === "exec" ? "#ffffff" : "#3b82f6"),
        2 / canvas.scale,
        gesture.startPin.direction === "input"
      );
    }

    ctx.restore();
  }, [nodes, canvas, gesture, pinOffsets, getPinWorldPos, getCanvasLocalPoint]);

  // 同步画布尺寸并触发重绘
  useLayoutEffect(() => {
    const canvasEl = edgeCanvasRef.current;
    if (!canvasEl || !ref.current) return;
    
    const rect = ref.current.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    
    // 设置实际像素大小 (防止模糊)
    canvasEl.width = rect.width * dpr;
    canvasEl.height = rect.height * dpr;
    // 设置 CSS 大小
    canvasEl.style.width = `${rect.width}px`;
    canvasEl.style.height = `${rect.height}px`;
    
    const ctx = canvasEl.getContext("2d");
    if (ctx) ctx.scale(dpr, dpr);
    
    drawAllEdges();
  }, [drawAllEdges]);

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

      if (!targetPinId) {
        // 需求 3: 连接线如果没有连接节点自动打开右键的菜单栏
        setContextMenu({
          x: e.clientX,
          y: e.clientY,
          visible: true
        });
        return;
      }

      if (targetPinId === sourcePin.id) return;

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

          if (n.selected !== isIntersecting) {
            changed = true;
            const newNode = n.clone();
            newNode.selected = isIntersecting;
            return newNode;
          }
          return n;
        });

        if (changed) {
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
            return nextNodes;
          }
          return prev;
        });
      }
    }
    prevSelectionRef.current = selection;
  }, [selection, canvas.x, canvas.y, canvas.scale]);

  const handleNodePointerDown = useCallback((id: string, e: React.PointerEvent) => {
    e.stopPropagation();
    
    // 如果是变量节点，自动在侧边栏显示其属性
    const clickedNode = nodes.find(n => n.id === id);
    if (clickedNode?.variableId) {
      setSelectedVariableId(clickedNode.variableId);
    } else {
      // 需求 5: 点击非变量节点时清除变量选中
      setSelectedVariableId(null);
    }

    // 如果没有按住 Ctrl/Shift，且当前节点未被选中，则清除其他选中项
    setNodes(nodes => {
      const alreadySelected = nodes.find(n => n.id === id)?.selected;
      if (alreadySelected) return nodes;

      return nodes.map(n => {
        if (n.id === id) {
          const newNode = n.clone();
          newNode.selected = true;
          return newNode;
        } else if (n.selected) {
          const newNode = n.clone();
          newNode.selected = false;
          return newNode;
        }
        return n;
      });
    });
  }, [setNodes, nodes, setSelectedVariableId]);

  const lastMousePosRef = useRef({ x: 0, y: 0 });
  const clipboardRef = useRef<BaseNode[]>([]);

  useEffect(() => {
    const handlePointerMove = (e: PointerEvent) => {
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    };
    window.addEventListener("pointermove", handlePointerMove, { capture: true });
    return () => window.removeEventListener("pointermove", handlePointerMove, { capture: true });
  }, []);

  const copySelectedNodes = useCallback(() => {
    const selectedNodes = nodes.filter(n => selectedNodeIds.has(n.id));
    if (selectedNodes.length === 0) return;

    // 深度克隆节点以保存当前状态
    clipboardRef.current = selectedNodes.map(n => n.clone());
  }, [nodes, selectedNodeIds]);

  const pasteNodes = useCallback(() => {
    let clipboard = clipboardRef.current;
    if (clipboard.length === 0) return;

    // 0. 过滤掉不安全的节点
    clipboard = clipboard.filter(node => {
      // 检查节点类型是否存在
      if (!NODE_REGISTRY.getDefinition(node.type)) return false;
      
      // 如果是变量相关节点，检查其引用的变量 ID 是否依然存在
      if (node.variableId && !variables[node.variableId]) {
        return false;
      }
      
      return true;
    });
    
    if (clipboard.length === 0) return;

    // 1. 获取鼠标位置对应的世界坐标
    const worldPos = getCanvasLocalPoint(lastMousePosRef.current.x, lastMousePosRef.current.y);

    // 2. 计算剪贴板中节点的包围盒左上角
    let minX = Infinity;
    let minY = Infinity;
    clipboard.forEach(node => {
      minX = Math.min(minX, node.position.x);
      minY = Math.min(minY, node.position.y);
    });

    const offsetX = worldPos.x - minX;
    const offsetY = worldPos.y - minY;

    // 3. 生成新 ID 映射
    const pinIdMap = new Map<string, string>();
    const newNodes = clipboard.map(node => {
      const newNode = node.clone();
      const newNodeId = `node-${crypto.randomUUID()}`;
      newNode.id = newNodeId;
      newNode.position = {
        x: node.position.x + offsetX,
        y: node.position.y + offsetY
      };
      newNode.selected = true; // 粘贴后选中

      // 更新 pins 的 ID 和 nodeId
      const updatePins = (pins: Pin[]) => {
        return pins.map(pin => {
          const oldId = pin.id;
          // 保持原有 ID 的后缀，以便识别
          const suffix = oldId.split('-').pop();
          const newId = `${newNodeId}-${pin.direction}-${suffix}-${crypto.randomUUID().slice(0, 8)}`;
          pin.id = newId;
          pin.nodeId = newNodeId;
          pinIdMap.set(oldId, newId);
          return pin;
        });
      };

      newNode.inputs = updatePins(newNode.inputs);
      newNode.outputs = updatePins(newNode.outputs);

      return newNode;
    });

    // 4. 修正连接线：仅保留粘贴节点之间的连接
    newNodes.forEach(node => {
      [...node.inputs, ...node.outputs].forEach(pin => {
        pin.links = pin.links
          .map(oldLinkId => pinIdMap.get(oldLinkId))
          .filter((newLinkId): newLinkId is string => !!newLinkId);
      });
    });

    // 5. 更新状态
    setNodes(prev => {
      // 取消之前的选中
      const next = prev.map(n => {
        if (n.selected) {
          const cloned = n.clone();
          cloned.selected = false;
          return cloned;
        }
        return n;
      });
      return [...next, ...newNodes];
    });
  }, [getCanvasLocalPoint, setNodes, variables]);

  const deleteSelectedNodes = useCallback(() => {
    const selectedIds = selectedNodeIdsRef.current;
    if (selectedIds.size === 0) return;

    setNodes((prev) => {
      // 1. 收集所有要删除节点的 Pin ID
      const pinsToDelete = new Set<string>();
      prev.forEach((node) => {
        if (selectedIds.has(node.id)) {
          node.inputs.forEach((p) => pinsToDelete.add(p.id));
          node.outputs.forEach((p) => pinsToDelete.add(p.id));
        }
      });

      // 2. 过滤掉被删除的节点，并清理剩余节点中的连接
      return prev
        .filter((node) => !selectedIds.has(node.id))
        .map((node) => {
          let nodeChanged = false;
          const newNode = node.clone();

          const cleanPins = (pins: Pin[]) => {
            pins.forEach(pin => {
              const originalLength = pin.links.length;
              pin.links = pin.links.filter(linkId => !pinsToDelete.has(linkId));
              if (pin.links.length !== originalLength) {
                nodeChanged = true;
              }
            });
          };

          cleanPins(newNode.inputs);
          cleanPins(newNode.outputs);

          return nodeChanged ? newNode : node;
        });
    });
  }, []);

  const cutSelectedNodes = useCallback(() => {
    copySelectedNodes();
    deleteSelectedNodes();
  }, [copySelectedNodes, deleteSelectedNodes]);

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
  }, [drag, variables]); // 增加 variables 依赖，确保 handleDropTemplate 使用最新状态

  // 全局记录修饰键状态
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;

      // 防止在输入框中触发快捷键
      const isInput = 
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA" ||
        (document.activeElement as HTMLElement)?.isContentEditable;

      if (isInput) return;

      if (e.key === "Delete" || e.key === "Backspace") {
        deleteSelectedNodes();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
        copySelectedNodes();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "x") {
        cutSelectedNodes();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v") {
        pasteNodes();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [deleteSelectedNodes, copySelectedNodes, pasteNodes]);

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
      // 安全检查：确保变量依然存在
      if (!variables[dragState.template.variableId]) {
        console.warn("Variable no longer exists. Aborting drop.");
        return;
      }

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
        variableId: dragState.template.variableId,
        variableName: dragState.template.variableName,
        variableType: dragState.template.variableType,
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
        {/* GPU 加速的连接线层 */}
        <canvas
          ref={edgeCanvasRef}
          className="absolute inset-0 pointer-events-none"
        />
        
        <div
          style={{
            transform: `translate(${canvas.x}px, ${canvas.y}px) scale(${canvas.scale})`,
            transformOrigin: "0 0",
          }}
        >
          {nodes.map((node) => (
            <Node
              key={node.id}
              node={node}
              scale={canvas.scale}
              onDrag={handleNodeDrag}
              onPointerDown={handleNodePointerDown}
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
              if (!variables[variableDropMenu.variableId]) {
                console.warn("Variable no longer exists.");
                setVariableDropMenu(null);
                return;
              }
              const newNode = createNodeFromTemplate(
                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                canvas.scale,
                "get_variable",
                { 
                  title: `Get ${variableDropMenu.variableName}`,
                  variableId: variableDropMenu.variableId,
                  variableType: variableDropMenu.variableType // 传入初始变量类型
                }
              );
              if (newNode) setNodes((prev) => [...prev, newNode]);
              setVariableDropMenu(null);
            }}
          >
            <div className="w-2 h-2 rounded-full bg-blue-400" />
            Get {variableDropMenu.variableName}
          </div>
          <div
            className="px-4 py-2 hover:bg-gray-600 cursor-pointer text-sm font-bold flex items-center gap-2 border-t border-gray-700"
            onClick={() => {
              if (!variables[variableDropMenu.variableId]) {
                console.warn("Variable no longer exists.");
                setVariableDropMenu(null);
                return;
              }
              const newNode = createNodeFromTemplate(
                { x: variableDropMenu.worldX, y: variableDropMenu.worldY },
                canvas.scale,
                "set_variable",
                { 
                  title: `Set ${variableDropMenu.variableName}`,
                  variableId: variableDropMenu.variableId,
                  variableType: variableDropMenu.variableType // 传入初始变量类型
                }
              );
              if (newNode) setNodes((prev) => [...prev, newNode]);
              setVariableDropMenu(null);
            }}
          >
            <div className="w-2 h-2 rounded-full bg-orange-400" />
            Set {variableDropMenu.variableName}
          </div>
        </div>
      )}
    </div>
  );
}
