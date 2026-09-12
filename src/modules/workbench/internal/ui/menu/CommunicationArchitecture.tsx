import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { IconType } from "react-icons";
import {
  FiActivity,
  FiArrowRight,
  FiBell,
  FiBox,
  FiCheckCircle,
  FiCode,
  FiInfo,
  FiMonitor,
  FiShuffle,
} from "react-icons/fi";
import { Handle, MarkerType, Position, type Edge, type Node, type NodeProps } from "@xyflow/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

export type CommunicationArchitectureNode = Node<
  {
    section: string;
    kind: "frontend" | "command" | "rust" | "contract" | "event" | "channel";
    color: string;
    icon: IconType;
    parts: string[];
    reference: string;
  },
  "communicationDetail"
>;

export const communicationNodes: CommunicationArchitectureNode[] = [
  {
    id: "service",
    type: "communicationDetail",
    position: { x: 0, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "service",
      kind: "frontend",
      color: "var(--chart-1)",
      icon: FiMonitor,
      parts: ["parameters", "parsing", "identity"],
      reference: "src/services/",
    },
  },
  {
    id: "command",
    type: "communicationDetail",
    position: { x: 440, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "command",
      kind: "command",
      color: "var(--chart-4)",
      icon: FiArrowRight,
      parts: ["request", "result", "channelArgument"],
      reference: "invokeCommand",
    },
  },
  {
    id: "transport",
    type: "communicationDetail",
    position: { x: 880, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "transport",
      kind: "command",
      color: "var(--chart-4)",
      icon: FiShuffle,
      parts: ["registry", "validation", "mapping"],
      reference: "yss-api",
    },
  },
  {
    id: "business",
    type: "communicationDetail",
    position: { x: 1320, y: 360 },
    style: { pointerEvents: "auto" },
    data: {
      section: "business",
      kind: "rust",
      color: "var(--chart-2)",
      icon: FiBox,
      parts: ["useCase", "authority", "blocking"],
      reference: "Application / Domain",
    },
  },
  {
    id: "contract",
    type: "communicationDetail",
    position: { x: 660, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "contract",
      kind: "contract",
      color: "var(--chart-3)",
      icon: FiCode,
      parts: ["wireTypes", "identity", "paging"],
      reference: "DTO / CommandError",
    },
  },
  {
    id: "event",
    type: "communicationDetail",
    position: { x: 660, y: 720 },
    style: { pointerEvents: "auto" },
    data: {
      section: "event",
      kind: "event",
      color: "var(--chart-2)",
      icon: FiBell,
      parts: ["commit", "subscription", "cleanup"],
      reference: "Event",
    },
  },
  {
    id: "publication",
    type: "communicationDetail",
    position: { x: 220, y: 720 },
    style: { pointerEvents: "auto" },
    data: {
      section: "publication",
      kind: "frontend",
      color: "var(--chart-1)",
      icon: FiCheckCircle,
      parts: ["deduplication", "revision", "snapshot"],
      reference: "publication revision",
    },
  },
  {
    id: "channel",
    type: "communicationDetail",
    position: { x: 1320, y: 720 },
    style: { pointerEvents: "auto" },
    data: {
      section: "channel",
      kind: "channel",
      color: "var(--chart-3)",
      icon: FiActivity,
      parts: ["progress", "execution", "diagnostics"],
      reference: "Channel",
    },
  },
];

export const communicationEdges: Edge[] = [
  {
    id: "invoke",
    source: "service",
    target: "command",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "dispatch",
    source: "command",
    target: "transport",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "useCase",
    source: "transport",
    target: "business",
    sourceHandle: "right-out",
    targetHandle: "left-in",
  },
  {
    id: "outcome",
    source: "business",
    target: "transport",
    sourceHandle: "left-out",
    targetHandle: "right-in",
  },
  {
    id: "encode",
    source: "transport",
    target: "contract",
    sourceHandle: "top-out",
    targetHandle: "bottom-in",
  },
  {
    id: "response",
    source: "contract",
    target: "service",
    sourceHandle: "left-out",
    targetHandle: "top-in",
  },
  {
    id: "notify",
    source: "transport",
    target: "event",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
  {
    id: "publishEvent",
    source: "event",
    target: "publication",
    sourceHandle: "left-out",
    targetHandle: "right-in",
  },
  {
    id: "publishReceipt",
    source: "service",
    target: "publication",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
  {
    id: "stream",
    source: "transport",
    target: "channel",
    sourceHandle: "bottom-out",
    targetHandle: "top-in",
  },
].map((edge) => {
  const color = communicationNodes.find((node) => node.id === edge.source)!.data.color;
  return {
    ...edge,
    type: "architecture",
    style: { stroke: color, strokeWidth: 1.4 },
    markerEnd: { type: MarkerType.ArrowClosed, color },
  };
});

export function CommunicationCard({ data }: NodeProps<CommunicationArchitectureNode>) {
  const { t } = useTranslation();
  const key = `architectureModal.communicationView.${data.section}`;
  const Icon = data.icon;

  return (
    <Card
      className="architecture-card communication-architecture-card nodrag nopan"
      data-section={data.section}
      style={{ "--architecture-color": data.color } as CSSProperties}
    >
      {Object.values(Position).map((position) => {
        const vertical = position === Position.Top || position === Position.Bottom;
        // Split opposing directions into separate lanes so request and result arrows remain visible.
        const outgoing = position === Position.Left ? "65%" : "35%";
        const incoming = position === Position.Right ? "65%" : "35%";
        return (
          <span key={position}>
            <Handle
              id={`${position}-in`}
              type="target"
              position={position}
              style={vertical ? { left: "50%" } : { top: incoming }}
              isConnectable={false}
            />
            <Handle
              id={`${position}-out`}
              type="source"
              position={position}
              style={vertical ? { left: "50%" } : { top: outgoing }}
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
            {t(`architectureModal.communicationView.${data.kind}Kind`)}
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
        <code className="text-[10px] text-muted-foreground">{data.reference}</code>
        <Popover>
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="gap-1.5"
              aria-label={t("architectureModal.communicationView.inspect", {
                name: t(`${key}.title`),
              })}
            >
              <FiInfo aria-hidden="true" className="size-3" />
              {t("architectureModal.communicationView.details")}
            </Button>
          </PopoverTrigger>
          <PopoverContent side="right" className="nowheel max-h-[65dvh] w-80 overflow-y-auto p-4">
            <h3 className="font-semibold">{t(`${key}.title`)}</h3>
            <p className="leading-relaxed text-muted-foreground">{t(`${key}.boundary`)}</p>
            {data.section === "contract" && (
              <>
                <pre className="rounded-lg border bg-muted/40 p-3 text-[11px] leading-relaxed">
                  <code>
                    {
                      '{\n  "code": "project_not_found",\n  "details": null,\n  "incidentId": null\n}'
                    }
                  </code>
                </pre>
                <dl className="space-y-2">
                  {["code", "details", "incidentId"].map((field) => (
                    <div key={field}>
                      <dt className="font-mono text-xs font-medium">{field}</dt>
                      <dd className="text-xs leading-relaxed text-muted-foreground">
                        {t(`${key}.fields.${field}`)}
                      </dd>
                    </div>
                  ))}
                </dl>
              </>
            )}
            {data.section === "channel" && (
              <ol className="flex flex-wrap items-center gap-1.5 text-xs">
                {["create", "bind", "receive", "end", "cleanup"].map((step, index) => (
                  <li key={step} className="flex items-center gap-1.5">
                    {index > 0 && (
                      <FiArrowRight aria-hidden="true" className="size-3 text-muted-foreground" />
                    )}
                    <Badge variant="outline" className="normal-case tracking-normal">
                      {t(`${key}.lifecycle.${step}`)}
                    </Badge>
                  </li>
                ))}
              </ol>
            )}
          </PopoverContent>
        </Popover>
      </CardFooter>
    </Card>
  );
}
