import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useExecutionStore } from "../stores";
import { ExecutionEvent } from "@/shared/types/editor";

/**
 * 监听后端执行事件并更新执行状态
 */
export function useExecutionVisualization() {
  const {
    startExecution,
    completeExecution,
    markNodeExecuting,
    markNodeCompleted,
    markNodeError,
    addActiveConnection,
    removeActiveConnection,
    markConnectionCompleted,
    reset,
  } = useExecutionStore();

  useEffect(() => {
    console.log("[useExecutionVisualization] Setting up execution event listener...");

    const unlisten = listen<ExecutionEvent>("execution-event", (event) => {
      const data = event.payload;
      console.log("[ExecutionEvent]", data);

      switch (data.type) {
        case "execution_start":
          console.log("[ExecutionEvent] Execution started");
          startExecution();
          break;

        case "execution_complete":
          console.log("[ExecutionEvent] Execution completed");
          completeExecution();
          // 立即重置状态，不保持执行状态
          reset();
          break;

        case "node_start":
          if (data.nodeId) {
            console.log("[ExecutionEvent] Node started:", data.nodeId);
            markNodeExecuting(data.nodeId);
          }
          break;

        case "node_complete":
          if (data.nodeId) {
            console.log("[ExecutionEvent] Node completed:", data.nodeId);
            markNodeCompleted(data.nodeId);
          }
          break;

        case "node_error":
          if (data.nodeId) {
            console.log("[ExecutionEvent] Node error:", data.nodeId);
            markNodeError(data.nodeId, data.error);
          }
          break;

        case "connection_active":
          if (data.fromPinId && data.toPinId) {
            console.log("[ExecutionEvent] Connection active:", data.fromPinId, "->", data.toPinId);
            addActiveConnection(data.fromPinId, data.toPinId);
            // 300ms 后移除激活状态
            setTimeout(() => {
              removeActiveConnection(data.fromPinId!, data.toPinId!);
            }, 300);
          }
          break;

        default:
          console.warn("[ExecutionEvent] Unknown event type:", data.type);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [
    startExecution,
    completeExecution,
    markNodeExecuting,
    markNodeCompleted,
    markNodeError,
    addActiveConnection,
    removeActiveConnection,
    markConnectionCompleted,
    reset,
  ]);
}
