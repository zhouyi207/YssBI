import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { IconType } from "react-icons";
import {
  FiActivity,
  FiBox,
  FiCode,
  FiCpu,
  FiDatabase,
  FiFolder,
  FiInfo,
  FiMessageSquare,
  FiPackage,
  FiShuffle,
} from "react-icons/fi";
import { Handle, MarkerType, Position, type Edge, type Node, type NodeProps } from "@xyflow/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

export type BackendArchitectureNode = Node<
  {
    section: string;
    kind: "business" | "transport" | "support";
    color: string;
    icon: IconType;
    parts: string[];
    crates: string[];
  },
  "backendSubsystem"
>;

// Ownership and representative collaboration follow docs/draft/backend.md, not a Cargo dependency graph.
export const backendNodes: BackendArchitectureNode[] = [
  {
    id: "support",
    type: "backendSubsystem",
    position: { x: 0, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "support",
      kind: "support",
      color: "var(--muted-foreground)",
      icon: FiCpu,
      parts: ["composition", "diagnostics", "platform", "utilities"],
      crates: [
        "yssbi",
        "yss-tracing",
        "yss-diagnostics",
        "yss-file-replace",
        "yss-window-state",
        "yss-canonical-hash",
        "yss-display-naming",
        "yss-path-display",
      ],
    },
  },
  {
    id: "transport",
    type: "backendSubsystem",
    position: { x: 0, y: 320 },
    style: { pointerEvents: "auto" },
    data: {
      section: "transport",
      kind: "transport",
      color: "var(--chart-4)",
      icon: FiShuffle,
      parts: ["commands", "mapping", "delivery"],
      crates: ["yss-api"],
    },
  },
  {
    id: "assistant",
    type: "backendSubsystem",
    position: { x: 440, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "assistant",
      kind: "business",
      color: "var(--chart-3)",
      icon: FiMessageSquare,
      parts: ["turns", "tools", "gateway", "adapters"],
      crates: [
        "yss-automation-contract",
        "yss-statistical-harness",
        "yss-agent-rig",
        "yss-statistical-harness-sqlite",
      ],
    },
  },
  {
    id: "application",
    type: "backendSubsystem",
    position: { x: 440, y: 320 },
    style: { pointerEvents: "auto" },
    data: {
      section: "application",
      kind: "business",
      color: "var(--chart-1)",
      icon: FiBox,
      parts: ["useCases", "coordination", "sessions", "adoption"],
      crates: ["yss-application"],
    },
  },
  {
    id: "plugins",
    type: "backendSubsystem",
    position: { x: 440, y: 640 },
    style: { pointerEvents: "auto" },
    data: {
      section: "plugins",
      kind: "business",
      color: "var(--chart-3)",
      icon: FiPackage,
      parts: ["installation", "processes", "tasks", "results"],
      crates: ["yss-plugin-runtime", "yss-plugin-protocol", "yss-plugin-sdk"],
    },
  },
  {
    id: "project",
    type: "backendSubsystem",
    position: { x: 880, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "project",
      kind: "business",
      color: "var(--chart-2)",
      icon: FiFolder,
      parts: ["identity", "resources", "commits", "discovery"],
      crates: [
        "yss-project",
        "yss-project-model",
        "yss-project-layout",
        "yss-project-identity",
        "yss-project-operation",
        "yss-project-change",
        "yss-project-history",
        "yss-resource-lifecycle",
        "yss-resource-naming",
        "yss-chart-document",
        "yss-project-filesystem",
        "yss-project-discovery",
        "yss-project-progress",
        "yss-project-registry",
        "yss-project-registry-contract",
        "yss-project-registry-sqlite",
        "yss-project-watcher",
        "yss-project-watcher-notify",
      ],
    },
  },
  {
    id: "graph",
    type: "backendSubsystem",
    position: { x: 880, y: 320 },
    style: { pointerEvents: "auto" },
    data: {
      section: "graph",
      kind: "business",
      color: "var(--chart-1)",
      icon: FiActivity,
      parts: ["documents", "semantics", "compilation", "execution"],
      crates: [
        "yss-graph-document",
        "yss-graph-document-edit",
        "yss-graph-protocol",
        "yss-graph-editor",
        "yss-graph-analysis",
        "yss-graph-analysis-contract",
        "yss-graph-resource-contract",
        "yss-graph-type-mapping",
        "yss-graph-catalog",
        "yss-graph-registry",
        "yss-graph-compiler",
        "yss-graph-compiler-diagnostics",
        "yss-graph-runtime",
        "yss-function-editor-projection",
        "yss-execution",
      ],
    },
  },
  {
    id: "data",
    type: "backendSubsystem",
    position: { x: 880, y: 640 },
    style: { pointerEvents: "auto" },
    data: {
      section: "data",
      kind: "business",
      color: "var(--chart-2)",
      icon: FiDatabase,
      parts: ["catalog", "runtime", "engine", "exchange"],
      crates: [
        "yss-data-contract",
        "yss-tabular-contract",
        "yss-relational-contract",
        "yss-database-contract",
        "yss-database-runtime",
        "yss-database-edit",
        "yss-database-schema",
        "yss-dataset-store",
        "yss-dataset-profile",
        "yss-datafusion",
        "yss-sql-source",
        "yss-tabular-arrow",
        "yss-tabular-io",
      ],
    },
  },
  {
    id: "science",
    type: "backendSubsystem",
    position: { x: 1320, y: 320 },
    style: { pointerEvents: "auto" },
    data: {
      section: "science",
      kind: "business",
      color: "var(--chart-4)",
      icon: FiCode,
      parts: ["contracts", "runtime", "algorithms", "mathematics"],
      crates: ["yss-sci-contract", "yss-sci-runtime", "yss-sci", "yss-linalg", "yss-math-expr"],
    },
  },
];

export const backendEdges: Edge[] = [
  {
    id: "assembly",
    source: "support",
    target: "transport",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
  {
    id: "useCase",
    source: "transport",
    target: "application",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "assistantEntry",
    source: "transport",
    target: "assistant",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "pluginEntry",
    source: "transport",
    target: "plugins",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "gateway",
    source: "assistant",
    target: "application",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
  {
    id: "hostCapabilities",
    source: "plugins",
    target: "application",
    sourceHandle: "top-out",
    targetHandle: "bottom-in",
  },
  {
    id: "projectUseCase",
    source: "application",
    target: "project",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "graphUseCase",
    source: "application",
    target: "graph",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "dataUseCase",
    source: "application",
    target: "data",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "inputData",
    source: "data",
    target: "graph",
    sourceHandle: "top-out",
    targetHandle: "bottom-in",
  },
  {
    id: "computation",
    source: "science",
    target: "graph",
    sourceHandle: "left-out",
    targetHandle: "right-in",
  },
].map((edge) => {
  const color = backendNodes.find((node) => node.id === edge.source)!.data.color;
  return {
    ...edge,
    type: "architecture",
    style: { stroke: color, strokeWidth: 1.4 },
    markerEnd: { type: MarkerType.ArrowClosed, color },
  };
});

export function BackendSubsystemCard({ data }: NodeProps<BackendArchitectureNode>) {
  const { t } = useTranslation();
  const key = `architectureModal.backendView.${data.section}`;
  const Icon = data.icon;

  return (
    <Card
      className="architecture-card backend-architecture-card nodrag nopan"
      style={{ "--architecture-color": data.color } as CSSProperties}
    >
      {Object.values(Position).map((position) => (
        <span key={position}>
          <Handle id={`${position}-in`} type="target" position={position} isConnectable={false} />
          <Handle id={`${position}-out`} type="source" position={position} isConnectable={false} />
        </span>
      ))}
      <CardHeader className="items-start gap-3 border-0 p-4 pb-2">
        <div className="architecture-icon">
          <Icon aria-hidden="true" className="size-5" />
        </div>
        <div className="min-w-0 flex-1">
          <Badge
            variant="outline"
            className="architecture-runtime mb-2 normal-case tracking-normal"
          >
            {t(`architectureModal.backendView.${data.kind}Kind`)}
          </Badge>
          <h2 className="text-sm leading-snug font-semibold">{t(`${key}.title`)}</h2>
        </div>
      </CardHeader>
      <CardContent className="flex-1 px-4 pt-1 pb-4">
        <p className="text-xs leading-relaxed text-muted-foreground">{t(`${key}.description`)}</p>
        <div className="mt-3 flex flex-wrap gap-1.5">
          {data.parts.map((part) => (
            <Badge
              key={part}
              variant="outline"
              className="architecture-subsystem normal-case tracking-normal"
            >
              {t(`${key}.parts.${part}`)}
            </Badge>
          ))}
        </div>
      </CardContent>
      <CardFooter className="architecture-footer justify-between px-3 py-2">
        <code className="text-[10px] text-muted-foreground">{data.crates[0]}</code>
        <Popover>
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="gap-1.5"
              aria-label={t("architectureModal.backendView.inspect", { name: t(`${key}.title`) })}
            >
              <FiInfo aria-hidden="true" className="size-3" />
              {t("architectureModal.backendView.crates", { count: data.crates.length })}
            </Button>
          </PopoverTrigger>
          <PopoverContent side="right" className="nowheel max-h-[65dvh] w-80 overflow-y-auto p-4">
            <h3 className="font-semibold">{t(`${key}.title`)}</h3>
            <p className="leading-relaxed text-muted-foreground">{t(`${key}.boundary`)}</p>
            <div className="flex flex-wrap gap-1.5">
              {data.crates.map((crate) => (
                <Badge key={crate} variant="outline" className="normal-case tracking-normal">
                  <code>{crate}</code>
                </Badge>
              ))}
            </div>
          </PopoverContent>
        </Popover>
      </CardFooter>
    </Card>
  );
}
