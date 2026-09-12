import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { IconType } from "react-icons";
import {
  FiActivity,
  FiBarChart2,
  FiDatabase,
  FiFolder,
  FiGitBranch,
  FiInfo,
  FiLayout,
  FiMessageSquare,
  FiPackage,
  FiSettings,
  FiShuffle,
} from "react-icons/fi";
import { Handle, MarkerType, Position, type Edge, type Node, type NodeProps } from "@xyflow/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

export type FrontendArchitectureNode = Node<
  {
    section: string;
    kind: "business" | "host" | "transport" | "support";
    color: string;
    icon: IconType;
    parts: string[];
    paths: string[];
  },
  "frontendSubsystem"
>;

// Capability ownership from frontend.md; connections illustrate collaboration, not an import graph.
export const frontendNodes: FrontendArchitectureNode[] = [
  {
    id: "support",
    type: "frontendSubsystem",
    position: { x: 0, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "support",
      kind: "support",
      color: "var(--muted-foreground)",
      icon: FiSettings,
      parts: ["preferences", "theme", "i18n", "components"],
      paths: [
        "src/modules/settings/",
        "src/features/core/settings/",
        "src/features/core/theme/",
        "src/app/i18n/",
        "src/components/ui/",
      ],
    },
  },
  {
    id: "workbench",
    type: "frontendSubsystem",
    position: { x: 0, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "workbench",
      kind: "host",
      color: "var(--chart-1)",
      icon: FiLayout,
      parts: ["composition", "registry", "layout", "templates"],
      paths: [
        "src/app/",
        "src/modules/workbench/",
        "src/modules/commands/",
        "src/modules/details/",
      ],
    },
  },
  {
    id: "plugins",
    type: "frontendSubsystem",
    position: { x: 0, y: 720 },
    style: { pointerEvents: "auto" },
    data: {
      section: "plugins",
      kind: "business",
      color: "var(--chart-5)",
      icon: FiPackage,
      parts: ["management", "projection", "contributions"],
      paths: [
        "src/modules/plugins/",
        "src/features/application/plugins/",
        "src/features/core/plugins/",
      ],
    },
  },
  {
    id: "chart",
    type: "frontendSubsystem",
    position: { x: 460, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "chart",
      kind: "business",
      color: "var(--chart-5)",
      icon: FiBarChart2,
      parts: ["document", "binding", "rendering", "lifecycle"],
      paths: ["src/modules/chart/", "src/features/application/chart/", "src/features/core/chart/"],
    },
  },
  {
    id: "project",
    type: "frontendSubsystem",
    position: { x: 460, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "project",
      kind: "business",
      color: "var(--chart-2)",
      icon: FiFolder,
      parts: ["lifecycle", "resources", "operations", "publication"],
      paths: [
        "src/modules/project-explorer/",
        "src/features/application/project/",
        "src/features/application/resource/",
      ],
    },
  },
  {
    id: "assistant",
    type: "frontendSubsystem",
    position: { x: 460, y: 720 },
    style: { pointerEvents: "auto" },
    data: {
      section: "assistant",
      kind: "business",
      color: "var(--chart-3)",
      icon: FiMessageSquare,
      parts: ["input", "projection", "approval", "coordination"],
      paths: ["src/modules/assistant/", "src/features/application/assistant/"],
    },
  },
  {
    id: "graph",
    type: "frontendSubsystem",
    position: { x: 920, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "graph",
      kind: "business",
      color: "var(--chart-1)",
      icon: FiGitBranch,
      parts: ["draft", "projection", "canvas", "execution"],
      paths: [
        "src/modules/graph-editor/",
        "src/modules/node-catalog/",
        "src/modules/problems/",
        "src/modules/results/",
        "src/modules/output/",
        "src/features/core/graphDraft/",
        "src/features/application/graphDraft/",
        "src/features/application/graphEditing/",
        "src/features/application/graphProjection/",
        "src/features/application/execution/",
        "src/features/application/editor/",
        "src/features/core/graphSession/",
        "src/features/core/dataStore/",
      ],
    },
  },
  {
    id: "data",
    type: "frontendSubsystem",
    position: { x: 1380, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "data",
      kind: "business",
      color: "var(--chart-2)",
      icon: FiDatabase,
      parts: ["browsing", "paging", "exchange", "lifecycle"],
      paths: [
        "src/modules/data-explorer/",
        "src/modules/database-editor/",
        "src/features/application/dataManagement/",
        "src/features/application/databaseEditor/",
      ],
    },
  },
  {
    id: "communication",
    type: "frontendSubsystem",
    position: { x: 1380, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "communication",
      kind: "transport",
      color: "var(--chart-4)",
      icon: FiShuffle,
      parts: ["invoke", "parsing", "streams"],
      paths: ["src/services/", "src/services/ipc/invokeCommand.ts"],
    },
  },
  {
    id: "feedback",
    type: "frontendSubsystem",
    position: { x: 1380, y: 720 },
    style: { pointerEvents: "auto" },
    data: {
      section: "feedback",
      kind: "business",
      color: "var(--chart-3)",
      icon: FiActivity,
      parts: ["logs", "subscription", "localization"],
      paths: [
        "src/modules/logs/",
        "src/features/application/log/",
        "src/features/application/observability/",
      ],
    },
  },
];

export const frontendEdges: Edge[] = [
  {
    id: "shared",
    source: "support",
    target: "workbench",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
  {
    id: "contributions",
    source: "plugins",
    target: "workbench",
    sourceHandle: "top-out",
    targetHandle: "bottom-in",
  },
  {
    id: "resourceHost",
    source: "workbench",
    target: "project",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "assistantHost",
    source: "workbench",
    target: "assistant",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "chartResource",
    source: "project",
    target: "chart",
    sourceHandle: "top-out",
    targetHandle: "bottom-in",
  },
  {
    id: "openGraph",
    source: "project",
    target: "graph",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "draftCoordination",
    source: "assistant",
    target: "graph",
    sourceHandle: "right-out",
    targetHandle: "bottom-in",
  },
  {
    id: "request",
    source: "graph",
    target: "communication",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "adoption",
    source: "communication",
    target: "graph",
    sourceHandle: "left-out",
    targetHandle: "right-in",
  },
  {
    id: "dataRequest",
    source: "data",
    target: "communication",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
  {
    id: "diagnosticSubscription",
    source: "feedback",
    target: "communication",
    sourceHandle: "top-out",
    targetHandle: "bottom-in",
  },
].map((edge) => {
  const color = frontendNodes.find((node) => node.id === edge.source)!.data.color;
  return {
    ...edge,
    type: "architecture",
    style: { stroke: color, strokeWidth: 1.4 },
    markerEnd: { type: MarkerType.ArrowClosed, color },
  };
});

export function FrontendSubsystemCard({ data }: NodeProps<FrontendArchitectureNode>) {
  const { t } = useTranslation();
  const key = `architectureModal.frontendView.${data.section}`;
  const Icon = data.icon;

  return (
    <Card
      className="architecture-card frontend-architecture-card nodrag nopan"
      data-section={data.section}
      style={{ "--architecture-color": data.color } as CSSProperties}
    >
      {Object.values(Position).map((position) => {
        const vertical = position === Position.Top || position === Position.Bottom;
        return (
          <span key={position}>
            <Handle
              id={`${position}-in`}
              type="target"
              position={position}
              style={
                vertical ? { left: "50%" } : { top: position === Position.Right ? "65%" : "35%" }
              }
              isConnectable={false}
            />
            <Handle
              id={`${position}-out`}
              type="source"
              position={position}
              style={
                vertical ? { left: "50%" } : { top: position === Position.Left ? "65%" : "35%" }
              }
              isConnectable={false}
            />
          </span>
        );
      })}
      <CardHeader className="items-start gap-3 border-0 p-4 pb-2">
        <div className="architecture-icon">
          <Icon aria-hidden="true" className="size-5" />
        </div>
        <div className="min-w-0 flex-1">
          <Badge
            variant="outline"
            className="architecture-runtime mb-2 normal-case tracking-normal"
          >
            {t(`architectureModal.frontendView.${data.kind}Kind`)}
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
        <code className="min-w-0 truncate text-[10px] text-muted-foreground" title={data.paths[0]}>
          {data.paths[0]}
        </code>
        <Popover>
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="shrink-0 gap-1.5"
              aria-label={t("architectureModal.frontendView.inspect", { name: t(`${key}.title`) })}
            >
              <FiInfo aria-hidden="true" className="size-3" />
              {t("architectureModal.frontendView.locations", { count: data.paths.length })}
            </Button>
          </PopoverTrigger>
          <PopoverContent
            side="right"
            className="nowheel max-h-[65dvh] w-96 max-w-[calc(100vw-2rem)] overflow-y-auto p-4"
          >
            <h3 className="font-semibold">{t(`${key}.title`)}</h3>
            <p className="leading-relaxed text-muted-foreground">{t(`${key}.boundary`)}</p>
            {data.section === "workbench" && (
              <dl className="space-y-3">
                {["template", "document", "snapshot"].map((item) => (
                  <div key={item}>
                    <dt className="text-xs font-medium">{t(`${key}.json.${item}.title`)}</dt>
                    <dd className="mt-1 text-xs leading-relaxed text-muted-foreground">
                      {t(`${key}.json.${item}.description`)}
                    </dd>
                  </div>
                ))}
              </dl>
            )}
            <div>
              <h4 className="mb-2 text-xs font-medium">
                {t("architectureModal.frontendView.representativeLocations")}
              </h4>
              <ul className="space-y-1.5">
                {data.paths.map((path) => (
                  <li
                    key={path}
                    className="rounded-md border border-border/60 bg-muted/30 px-2 py-1.5"
                  >
                    <code className="text-[11px] break-all">{path}</code>
                  </li>
                ))}
              </ul>
            </div>
          </PopoverContent>
        </Popover>
      </CardFooter>
    </Card>
  );
}
