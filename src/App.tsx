import "./App.css";
import { useState, useRef, useEffect } from "react";
// import { Canvas } from "./components/Canvas";
import { NodeProps } from "./components/Node";
import { Connection } from "./components/Edge";
import InfiniteCanvas from "./components/Canvas";

export default function App() {
  const [components, setComponents] = useState<NodeProps[]>([]);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [selectedComponent, setSelectedComponent] = useState<string | null>(
    null
  );
  const [connectionSource, setConnectionSource] = useState<string | null>(null);
  const [canvasOffset, setCanvasOffset] = useState({ x: 0, y: 0 });

  const dragState = useRef({
    isDraggingComponent: false,
    componentId: null as string | null,
    offsetX: 0,
    offsetY: 0,
    isDraggingCanvas: false,
    canvasStartX: 0,
    canvasStartY: 0,
  });

  const canvasRef = useRef<HTMLDivElement>(null);

  // 添加新组件
  const addComponent = () => {
    const newComponent: NodeProps = {
      id: `comp-${Date.now()}`,
      x: 100 + Math.random() * 400,
      y: 100 + Math.random() * 300,
      width: 150,
      height: 80,
      title: `组件 ${components.length + 1}`,
      type: "component",
      inputs: [],
      outputs: [],
    };
    setComponents([...components, newComponent]);
  };

  // 节点按下
  const handleComponentMouseDown = (
    e: React.MouseEvent,
    componentId: string
  ) => {
    e.stopPropagation();
    const component = components.find((c) => c.id === componentId);
    if (component && canvasRef.current) {
      const rect = canvasRef.current.getBoundingClientRect();
      const mouseX = e.clientX - rect.left - canvasOffset.x;
      const mouseY = e.clientY - rect.top - canvasOffset.y;

      dragState.current.isDraggingComponent = true;
      dragState.current.componentId = componentId;
      dragState.current.offsetX = mouseX - component.x;
      dragState.current.offsetY = mouseY - component.y;

      setSelectedComponent(componentId);
    }
  };

  // 画布按下
  const handleCanvasMouseDown = (e: React.MouseEvent) => {
    if (!dragState.current.isDraggingComponent) {
      dragState.current.isDraggingCanvas = true;
      dragState.current.canvasStartX = e.clientX;
      dragState.current.canvasStartY = e.clientY;
      if (canvasRef.current) canvasRef.current.style.cursor = "grabbing";
    }
  };

  // 鼠标移动
  const handleMouseMove = (e: MouseEvent) => {
    if (
      dragState.current.isDraggingComponent &&
      dragState.current.componentId
    ) {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;

      const mouseX = e.clientX - rect.left - canvasOffset.x;
      const mouseY = e.clientY - rect.top - canvasOffset.y;

      const newX = mouseX - dragState.current.offsetX;
      const newY = mouseY - dragState.current.offsetY;

      setComponents((prev) =>
        prev.map((comp) =>
          comp.id === dragState.current.componentId
            ? { ...comp, x: newX, y: newY }
            : comp
        )
      );
    } else if (dragState.current.isDraggingCanvas) {
      const deltaX = e.clientX - dragState.current.canvasStartX;
      const deltaY = e.clientY - dragState.current.canvasStartY;

      setCanvasOffset((prev) => ({
        x: prev.x + deltaX,
        y: prev.y + deltaY,
      }));

      dragState.current.canvasStartX = e.clientX;
      dragState.current.canvasStartY = e.clientY;
    }
  };

  // 鼠标释放
  const handleMouseUp = () => {
    dragState.current.isDraggingComponent = false;
    dragState.current.componentId = null;
    dragState.current.isDraggingCanvas = false;
    setSelectedComponent(null);

    if (canvasRef.current) canvasRef.current.style.cursor = "grab";
  };

  // 点击连接点
  const handleConnectionPointClick = (
    componentId: string,
    type: "input" | "output"
  ) => {
    if (type === "output") {
      setConnectionSource(componentId);
    } else if (type === "input" && connectionSource) {
      const newConnection: Connection = {
        id: `conn-${Date.now()}`,
        from: connectionSource,
        to: componentId,
      };
      setConnections((prev) => [...prev, newConnection]);
      setConnectionSource(null);
    }
  };

  // 全局鼠标事件
  useEffect(() => {
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [canvasOffset, components]);

  return (
    <div className="flex flex-col w-full h-screen">
      <div className="flex items-center h-12 bg-gray-800 text-white px-4">
        <button className="px-3 py-1 bg-blue-600 rounded hover:bg-blue-700">
          添加组件
        </button>
      </div>
      <div className="flex-1 relative">
        {/* <Canvas
          selectedComponent={selectedComponent}
          connectionSource={connectionSource}
          canvasOffset={canvasOffset}
          onComponentMouseDown={handleComponentMouseDown}
          onConnectionPointClick={handleConnectionPointClick}
          onCanvasMouseDown={handleCanvasMouseDown}
        /> */}
        <InfiniteCanvas/>
      </div>
    </div>
  );
}
