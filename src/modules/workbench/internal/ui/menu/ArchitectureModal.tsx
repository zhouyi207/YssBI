import { useEffect, useMemo, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { Link, useLocation, useNavigate } from "react-router";
import type { IconType } from "react-icons";
import {
  FiActivity,
  FiArrowRight,
  FiBox,
  FiCode,
  FiChevronRight,
  FiDatabase,
  FiGrid,
  FiLayers,
  FiMessageSquare,
  FiMonitor,
  FiMousePointer,
  FiServer,
  FiShield,
  FiShuffle,
} from "react-icons/fi";
import { VscClose, VscTypeHierarchy } from "react-icons/vsc";
import {
  Background,
  BaseEdge,
  Controls,
  EdgeLabelRenderer,
  getBezierPath,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  ReactFlow,
  useNodesInitialized,
  useReactFlow,
  useStore,
  useViewport,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { architectureSearch, readArchitectureView } from "./architectureNavigation";
import {
  backendEdges,
  backendNodes,
  BackendSubsystemCard,
  type BackendArchitectureNode,
} from "./BackendArchitecture";
import {
  communicationEdges,
  communicationNodes,
  CommunicationCard,
  type CommunicationArchitectureNode,
} from "./CommunicationArchitecture";
import {
  frontendEdges,
  frontendNodes,
  FrontendSubsystemCard,
  type FrontendArchitectureNode,
} from "./FrontendArchitecture";
import "@xyflow/react/dist/base.css";
import "./architecture.css";

type ArchitectureNode = Node<
  {
    section: "frontend" | "communication" | "backend";
    technology: string;
    runtime: string;
    color: string;
    icon: IconType;
    items: { key: string; icon: IconType }[];
  },
  "architecture"
>;

type DiagramNode =
  | ArchitectureNode
  | BackendArchitectureNode
  | CommunicationArchitectureNode
  | FrontendArchitectureNode;

// Fixed overview from docs/draft/architecture.md; no project graph or runtime state is involved.
const nodes: ArchitectureNode[] = [
  {
    id: "frontend",
    type: "architecture",
    position: { x: 0, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "frontend",
      technology: "React",
      runtime: "WebView",
      color: "var(--chart-1)",
      icon: FiMonitor,
      items: [
        { key: "presentation", icon: FiGrid },
        { key: "interaction", icon: FiMousePointer },
        { key: "state", icon: FiLayers },
        { key: "feedback", icon: FiMessageSquare },
      ],
    },
  },
  {
    id: "communication",
    type: "architecture",
    position: { x: 490, y: 28 },
    style: { pointerEvents: "auto" },
    data: {
      section: "communication",
      technology: "Tauri IPC",
      runtime: "Bridge",
      color: "var(--chart-4)",
      icon: FiShuffle,
      items: [
        { key: "command", icon: FiArrowRight },
        { key: "event", icon: FiMessageSquare },
        { key: "channel", icon: FiActivity },
      ],
    },
  },
  {
    id: "backend",
    type: "architecture",
    position: { x: 980, y: 0 },
    style: { pointerEvents: "auto" },
    data: {
      section: "backend",
      technology: "Rust",
      runtime: "Native",
      color: "var(--chart-2)",
      icon: FiServer,
      items: [
        { key: "orchestration", icon: FiBox },
        { key: "domain", icon: FiCode },
        { key: "state", icon: FiShield },
        { key: "persistence", icon: FiDatabase },
      ],
    },
  },
];

const detailDiagrams = {
  frontend: { nodes: frontendNodes, edges: frontendEdges },
  backend: { nodes: backendNodes, edges: backendEdges },
  communication: { nodes: communicationNodes, edges: communicationEdges },
};
const subsystems = ["application", "project", "graph", "data", "science", "assistant", "plugins"];
const fitViewOptions = { padding: 0.1, minZoom: 0.2, maxZoom: 1.1 };
const edgeOptions = { type: "architecture", interactionWidth: 0 };

function ArchitectureCard({ data }: NodeProps<ArchitectureNode>) {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const detailLocation = { ...location, search: architectureSearch(location.search, data.section) };
  const sectionKey = `architectureModal.${data.section}`;
  const Icon = data.icon;
  const transport = data.section === "communication";

  return (
    <Card
      className="architecture-card architecture-drilldown nodrag nopan"
      data-section={data.section}
      style={{ "--architecture-color": data.color } as CSSProperties}
      onClick={() => navigate(detailLocation)}
    >
      {data.section !== "frontend" && (
        <>
          <Handle
            id="request-in"
            type="target"
            position={Position.Left}
            className="architecture-port-request"
            isConnectable={false}
          />
          <Handle
            id="response-out"
            type="source"
            position={Position.Left}
            className="architecture-port-response"
            isConnectable={false}
          />
        </>
      )}
      <CardHeader className="flex-col items-stretch gap-4 border-0 px-5 pt-5 pb-4">
        <div className="flex items-center gap-3">
          <div className="architecture-icon">
            <Icon aria-hidden="true" className="size-5" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="mb-1 text-[11px] font-medium text-muted-foreground">
              {t(`${sectionKey}.title`)}
            </p>
            <h2 className="text-[22px] leading-none font-semibold tracking-tight">
              {data.technology}
            </h2>
          </div>
          <Badge variant="outline" className="architecture-runtime normal-case tracking-normal">
            {data.runtime}
          </Badge>
        </div>
        <p className="text-xs leading-relaxed text-muted-foreground">
          {t(`${sectionKey}.description`)}
        </p>
      </CardHeader>
      <Separator className="architecture-divider" />
      <CardContent className={`grid gap-2 p-4 ${transport ? "grid-cols-1" : "grid-cols-2"}`}>
        {data.items.map(({ key, icon: ItemIcon }) => (
          <Tooltip key={key} delayDuration={250}>
            <TooltipTrigger asChild>
              <div
                tabIndex={0}
                className={`architecture-capability ${transport ? "architecture-transport" : ""}`}
              >
                <div className="flex items-center gap-2">
                  <ItemIcon
                    aria-hidden="true"
                    className="size-3.5 shrink-0 text-[var(--architecture-color)]"
                  />
                  <h3 className="text-xs font-medium leading-relaxed">
                    {t(`${sectionKey}.${key}.title`)}
                  </h3>
                </div>
                <p className="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
                  {t(`${sectionKey}.${key}.summary`)}
                </p>
              </div>
            </TooltipTrigger>
            <TooltipContent
              side="bottom"
              sideOffset={6}
              className="z-[530] max-w-72 px-3 py-2 leading-relaxed"
            >
              {t(`${sectionKey}.${key}.description`)}
            </TooltipContent>
          </Tooltip>
        ))}
      </CardContent>
      <CardFooter className="architecture-footer block px-4 py-3.5">
        <Tooltip delayDuration={250}>
          <TooltipTrigger asChild>
            <div tabIndex={0} className="architecture-boundary">
              <div className="flex items-center gap-2 text-[11px] font-medium">
                <FiShield
                  aria-hidden="true"
                  className="size-3.5 shrink-0 text-[var(--architecture-color)]"
                />
                <h3>{t(`${sectionKey}.boundaryTitle`)}</h3>
              </div>
              {data.section === "backend" ? (
                <div className="mt-2.5 flex flex-wrap gap-1.5">
                  {subsystems.map((subsystem) => (
                    <Badge
                      key={subsystem}
                      variant="outline"
                      className="architecture-subsystem normal-case tracking-normal"
                    >
                      {t(`${sectionKey}.subsystems.${subsystem}`)}
                    </Badge>
                  ))}
                </div>
              ) : transport ? (
                <code className="mt-2 block text-[10px] leading-relaxed text-muted-foreground">
                  {"{ code, details, incidentId }"}
                </code>
              ) : (
                <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                  {t(`${sectionKey}.boundarySummary`)}
                </p>
              )}
            </div>
          </TooltipTrigger>
          <TooltipContent
            side="bottom"
            sideOffset={6}
            className="z-[530] block max-w-80 px-3 py-2 leading-relaxed"
          >
            {transport && <p className="mb-2">{t(`${sectionKey}.contract.description`)}</p>}
            <p>{t(`${sectionKey}.boundary`)}</p>
          </TooltipContent>
        </Tooltip>
      </CardFooter>
      <Button
        asChild
        variant="ghost"
        className="mx-3 mb-3 justify-between text-[var(--architecture-color)]"
      >
        <Link to={detailLocation} onClick={(event) => event.stopPropagation()}>
          {t(
            data.section === "frontend"
              ? "architectureModal.openFrontend"
              : transport
                ? "architectureModal.openCommunication"
                : "architectureModal.openBackend",
          )}
          <FiArrowRight aria-hidden="true" className="size-3.5" />
        </Link>
      </Button>
      {data.section !== "backend" && (
        <>
          <Handle
            id="request-out"
            type="source"
            position={Position.Right}
            className="architecture-port-request"
            isConnectable={false}
          />
          <Handle
            id="response-in"
            type="target"
            position={Position.Right}
            className="architecture-port-response"
            isConnectable={false}
          />
        </>
      )}
    </Card>
  );
}

function ArchitectureConnection({ id, label, markerEnd, style, ...positions }: EdgeProps) {
  const [path, labelX, labelY] = getBezierPath(positions);
  const vertical =
    positions.sourcePosition === Position.Top || positions.sourcePosition === Position.Bottom;
  return (
    <>
      <path d={path} fill="none" stroke={style?.stroke} strokeWidth={7} opacity={0.07} />
      <BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} interactionWidth={0} />
      {[0, 1.6].map((delay) => (
        <g key={delay} className="architecture-flow-arrow" aria-hidden="true">
          <path
            d="M -5 -4 L 0 0 L -5 4"
            fill="none"
            stroke={style?.stroke}
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <animateMotion
            dur="3.2s"
            begin={`-${delay}s`}
            repeatCount="indefinite"
            rotate="auto"
            path={path}
          />
        </g>
      ))}
      <EdgeLabelRenderer>
        <Badge
          variant="outline"
          className="architecture-edge-label normal-case tracking-normal"
          style={{
            transform: `translate(-50%, -50%) translate(${labelX + (vertical ? 70 : 0)}px, ${labelY - (vertical ? 0 : 17)}px)`,
          }}
        >
          {label}
        </Badge>
      </EdgeLabelRenderer>
    </>
  );
}

const nodeTypes = {
  architecture: ArchitectureCard,
  frontendSubsystem: FrontendSubsystemCard,
  backendSubsystem: BackendSubsystemCard,
  communicationDetail: CommunicationCard,
};
const edgeTypes = { architecture: ArchitectureConnection };

function ArchitectureViewport() {
  const { t } = useTranslation();
  const { fitView } = useReactFlow();
  const { zoom } = useViewport();
  const width = useStore((state) => state.width);
  const height = useStore((state) => state.height);
  const initialized = useNodesInitialized();

  useEffect(() => {
    if (initialized) void fitView(fitViewOptions);
  }, [fitView, height, initialized, width]);

  return (
    <>
      <Controls orientation="horizontal" showInteractive={false} fitViewOptions={fitViewOptions}>
        <span className="architecture-zoom tabular-nums">{Math.round(zoom * 100)}%</span>
      </Controls>
      <MiniMap<DiagramNode>
        className="architecture-minimap"
        style={{ width: 140, height: 82 }}
        nodeColor={(node) => node.data.color}
        nodeBorderRadius={8}
        bgColor="transparent"
        maskColor="color-mix(in srgb, var(--workbench-bg) 65%, transparent)"
        maskStrokeColor="var(--border)"
        ariaLabel={t("architectureModal.minimap")}
        pannable
        zoomable
      />
    </>
  );
}

export function ArchitectureModal() {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const view = readArchitectureView(location.search);
  const isBackend = view === "backend";
  const detail = view && view !== "overview" ? detailDiagrams[view] : null;
  const detailKey = `${view}View`;
  const edges = useMemo<Edge[]>(
    () =>
      detail
        ? detail.edges.map((edge) => ({
            ...edge,
            label: t(`architectureModal.${detailKey}.links.${edge.id}`),
          }))
        : [
            {
              id: "request",
              source: "frontend",
              target: "communication",
              sourceHandle: "request-out",
              targetHandle: "request-in",
              label: t("architectureModal.request"),
              style: { stroke: "var(--chart-1)", strokeWidth: 1.4 },
              markerEnd: { type: MarkerType.ArrowClosed, color: "var(--chart-1)" },
            },
            {
              id: "dispatch",
              source: "communication",
              target: "backend",
              sourceHandle: "request-out",
              targetHandle: "request-in",
              label: t("architectureModal.dispatch"),
              style: { stroke: "var(--chart-4)", strokeWidth: 1.4 },
              markerEnd: { type: MarkerType.ArrowClosed, color: "var(--chart-4)" },
            },
            {
              id: "result",
              source: "backend",
              target: "communication",
              sourceHandle: "response-out",
              targetHandle: "response-in",
              label: t("architectureModal.result"),
              style: { stroke: "var(--chart-2)", strokeWidth: 1.4 },
              markerEnd: { type: MarkerType.ArrowClosed, color: "var(--chart-2)" },
            },
            {
              id: "feedback",
              source: "communication",
              target: "frontend",
              sourceHandle: "response-out",
              targetHandle: "response-in",
              label: t("architectureModal.feedback"),
              style: { stroke: "var(--chart-4)", strokeWidth: 1.4 },
              markerEnd: { type: MarkerType.ArrowClosed, color: "var(--chart-4)" },
            },
          ],
    [detailKey, detail, t],
  );

  return (
    <Dialog
      open={view !== null}
      onOpenChange={(open) => {
        if (!open)
          void navigate(
            { ...location, search: architectureSearch(location.search, null) },
            { replace: true },
          );
      }}
    >
      <DialogContent className="flex h-[86dvh] w-[92vw] max-w-none flex-col rounded-md p-0 motion-reduce:animate-none">
        <div className="flex h-9 shrink-0 items-center justify-between border-b border-border bg-[var(--sidebar-bg)] pr-2.5 pl-3.5">
          <div className="flex items-center gap-2">
            <VscTypeHierarchy aria-hidden="true" className="size-3.5 text-muted-foreground" />
            <DialogTitle className="text-xs font-medium normal-case tracking-normal">
              {t("menubar.architecture")}
            </DialogTitle>
          </div>
          <DialogClose asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-[26px] w-7 rounded-sm text-muted-foreground"
              aria-label={t("common.close")}
            >
              <VscClose aria-hidden="true" />
            </Button>
          </DialogClose>
        </div>
        <DialogDescription className="sr-only">
          {t("architectureModal.description")}
        </DialogDescription>
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border/60 px-5 py-3">
          <div className="flex items-center gap-2.5">
            <FiLayers aria-hidden="true" className="size-4 text-muted-foreground" />
            <nav aria-label={t("architectureModal.breadcrumb")}>
              <ol className="flex items-center gap-1.5 text-xs">
                <li>
                  {detail ? (
                    <Button asChild variant="ghost" size="sm">
                      <Link
                        to={{
                          ...location,
                          search: architectureSearch(location.search, "overview"),
                        }}
                      >
                        {t("architectureModal.overview")}
                      </Link>
                    </Button>
                  ) : (
                    <span aria-current="page" className="font-medium">
                      {t("architectureModal.overview")}
                    </span>
                  )}
                </li>
                {detail && (
                  <>
                    <li aria-hidden="true">
                      <FiChevronRight className="size-3.5 text-muted-foreground" />
                    </li>
                    <li>
                      <span aria-current="page" className="font-medium">
                        {t(`architectureModal.${view}Page`)}
                      </span>
                    </li>
                  </>
                )}
              </ol>
            </nav>
            <Badge variant="outline" className="normal-case tracking-normal">
              {isBackend
                ? t("architectureModal.backendView.inventory", {
                    systems: backendNodes.length,
                    count: backendNodes.reduce((count, node) => count + node.data.crates.length, 0),
                  })
                : detail
                  ? t(`architectureModal.${detailKey}.summary`, { count: detail.nodes.length })
                  : "Tauri Desktop"}
            </Badge>
          </div>
          <p className="text-[11px] text-muted-foreground">
            {t("architectureModal.navigationHint")}
          </p>
        </div>
        {detail && (
          <p className="shrink-0 border-b border-border/40 px-5 py-2 text-[11px] leading-relaxed text-muted-foreground">
            {t(`architectureModal.${detailKey}.caption`)}
          </p>
        )}
        <div className="architecture-surface min-h-0 flex-1">
          <ReactFlow<DiagramNode>
            key={view}
            className="architecture-flow"
            nodes={detail?.nodes ?? nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            defaultEdgeOptions={edgeOptions}
            fitView
            fitViewOptions={fitViewOptions}
            minZoom={0.2}
            maxZoom={1.8}
            nodesDraggable={false}
            nodesConnectable={false}
            nodesFocusable={false}
            edgesFocusable={false}
            edgesReconnectable={false}
            elementsSelectable={false}
            zoomOnDoubleClick={false}
            deleteKeyCode={null}
            selectionKeyCode={null}
            proOptions={{ hideAttribution: true }}
            aria-label={t("architectureModal.description")}
            ariaLabelConfig={{
              "controls.ariaLabel": t("architectureModal.controls"),
              "controls.zoomIn.ariaLabel": t("architectureModal.zoomIn"),
              "controls.zoomOut.ariaLabel": t("architectureModal.zoomOut"),
              "controls.fitView.ariaLabel": t("architectureModal.fitView"),
            }}
          >
            <Background
              color="color-mix(in srgb, var(--muted-foreground) 20%, transparent)"
              gap={24}
              size={1}
            />
            <ArchitectureViewport />
          </ReactFlow>
        </div>
      </DialogContent>
    </Dialog>
  );
}
