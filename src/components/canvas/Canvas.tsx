import { useRef, useState, useEffect } from "react";
import { Node } from "../node/Node";
import { BaseNode } from "../node/models";
import { useDrag } from "../drag/DragContext";
import { useCanvas } from "./CanvasContext";
import { createNodeFromTemplate } from "../node/util";
import HUD from "./HUD";
import NodePalette from "./NodePalette";

/* ================= Canvas ================= */

export default function InfiniteCanvas() {
  const { canvas, onCanvasWheel, onCanvasPointerDown, selection, contextMenu, setContextMenu } = useCanvas();
  const { drag } = useDrag();

  const [nodes, setNodes] = useState<BaseNode[]>([]);
  const [selectedNodeIds, setSelectedNodeIds] = useState<Set<string>>(new Set());
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
      nodes.forEach((node) => {
        const nodeWidth = node.noHeader ? 80 : 150;
        const nodeHeight = node.noHeader ? 60 : 100;
        if (
          node.position.x < box.x2 &&
          node.position.x + nodeWidth > box.x1 &&
          node.position.y < box.y2 &&
          node.position.y + nodeHeight > box.y1
        ) {
          newSelected.add(node.id);
        }
      });
      
      setNodes(prev => prev.map(n => {
        const isSelected = newSelected.has(n.id);
        if (n.selected !== isSelected) {
          const newNode = n.clone();
          newNode.selected = isSelected;
          return newNode;
        }
        return n;
      }));
      setSelectedNodeIds(newSelected);
    } else if (prevSelectionRef.current && !selection) {
      const s = prevSelectionRef.current;
      if (Math.abs(s.startX - s.currentX) < 5 && Math.abs(s.startY - s.currentY) < 5) {
        setNodes(prev => prev.map(n => { 
          if (n.selected) {
            const newNode = n.clone();
            newNode.selected = false;
            return newNode;
          }
          return n;
        }));
        setSelectedNodeIds(new Set());
      }
    }
    prevSelectionRef.current = selection;
  }, [selection, nodes.length, canvas.x, canvas.y, canvas.scale]);

  // 2. 多节点拖拽回调
  const handleNodeDrag = (id: string, dx: number, dy: number) => {
    setNodes((prev) =>
      prev.map((node) => {
        if (selectedNodeIds.has(id)) {
          if (selectedNodeIds.has(node.id)) {
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
        return node; // 关键优化：返回原引用，配合 React.memo 防止重绘
      })
    );
  };

  // 3. 动态添加输入
  const handleNodeAddInput = (id: string) => {
    setNodes((prev) =>
      prev.map((node) => {
        if (node.id === id) {
          const newNode = node.clone();
          const newIndex = newNode.inputs.length;
          newNode.addInput({
            id: `${id}-input-${newIndex}-${Date.now()}`,
            name: String.fromCharCode(65 + newIndex),
            type: "int", // 修改这里：PinType 中已改为 int
            direction: "input",
            connectedTo: [],
          });
          return newNode;
        }
        return node;
      })
    );
  };

  const handlePinClick = (pinId: string, _direction: "input" | "output") => {
    console.log(`Pin clicked: ${pinId}`);
  };

  useEffect(() => {
    // 从「有拖拽」→「无拖拽」 = drop
    if (prevDragRef.current && !drag) {
      const last = prevDragRef.current;

      if (last.type === "node-template") {
        handleDropTemplate(last);
      }
    }

    prevDragRef.current = drag;
  }, [drag]);

  /* ===== Data Drag ===== */
  function handleDropTemplate(dragState: any) {
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

    // 如果是变量，显示 Get/Set 选择菜单
    if (dragState.template.category === "Variable") {
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
        className="absolute inset-0"
        style={{
          backgroundImage: `
            linear-gradient(#333 1px, transparent 1px),
            linear-gradient(90deg, #333 1px, transparent 1px)
          `,
          backgroundSize: `${GRID * canvas.scale}px ${GRID * canvas.scale}px`,
          backgroundPosition: `${canvas.x}px ${canvas.y}px`, // 修正：移除负号，与 World 变换保持一致
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
          {nodes.map((node) => (
            <Node
              key={node.id}
              node={node}
              scale={canvas.scale}
              onDrag={handleNodeDrag}
              onAddInput={handleNodeAddInput}
              onPinClick={handlePinClick}
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
