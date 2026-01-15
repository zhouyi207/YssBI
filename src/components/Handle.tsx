import React from "react";

export interface HandleProps {
  id: string; // pin id
  componentId: string; // node id

  direction: "input" | "output";
  kind: "data" | "exec";

  dataType: string; // number / vector / bool / flow
  acceptTypes?: string[];

  order: number;
  connectionCount: number;
  connectionLimit?: number;

  title: string;

  defaultValue?: any;
  value?: any;

  required?: boolean;

  ui?: {
    color?: string;
    icon?: string;
    hidden?: boolean;
  };

  meta?: {
    description?: string;
  };

  onHandleClick?: (componentId: string, type: "input" | "output") => void;
}

export const Handle: React.FC<HandleProps> = ({
  id,
  componentId,
  direction,
  kind,
  dataType,
  acceptTypes,
  order,
  connectionCount,
  connectionLimit = kind === "exec" ? Infinity : 1,
  title,
  defaultValue,
  value,
  required,
  ui,
  meta,
  onHandleClick,
}) => {
  // ===== derived state =====
  const isConnected = connectionCount > 0;
  const isFull = direction === "input" && connectionCount >= connectionLimit;

  const color =
    ui?.color ??
    (kind === "exec"
      ? "#ffffff"
      : {
          number: "#60a5fa",
          string: "#34d399",
          boolean: "#facc15",
          vector: "#a78bfa",
          flow: "#ffffff",
        }[dataType] ?? "#9ca3af");

  return (
    <div
      className={`
        relative flex items-center
        ${direction === "input" ? "justify-start" : "justify-end"}
      `}
      style={{ order }}
    >
      {/* pin */}
      <div
        className={`
          w-3 h-3 rounded-full cursor-pointer
          ${isConnected ? "ring-1 ring-yellow-300" : ""}
          ${isFull ? "opacity-40 cursor-not-allowed" : ""}
        `}
        style={{
          backgroundColor: kind === "exec" ? "#fff" : color,
          border:
            kind === "exec" ? "1px solid #6b7280" : "1px solid transparent",
        }}
        title={`${title} (${dataType})`}
        onClick={(e) => {
          e.stopPropagation();
          if (isFull) return;
          onHandleClick?.(id, direction);
        }}
      >
        {/* required marker */}
        {required && !isConnected && (
          <span className="absolute -top-1 -right-1 w-2 h-2 bg-red-500 rounded-full" />
        )}
      </div>

      {/* label */}
      <span
        className={`
          text-xs text-gray-700 select-none
          ${direction === "input" ? "ml-2" : "mr-2"}
        `}
      >
        {title}
      </span>
    </div>
  );
};
