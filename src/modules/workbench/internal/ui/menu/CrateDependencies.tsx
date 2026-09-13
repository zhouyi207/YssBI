import { useMemo, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router";
import {
  Background,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  ReactFlow,
  type Node,
  type NodeProps,
  type Edge,
} from "@xyflow/react";
import { FiPackage } from "react-icons/fi";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import crates from "./crateDependencies.json";

type CrateNode = Node<
  {
    name: string;
    color: string;
    manifest: string;
    focused: boolean;
    incoming: number;
    outgoing: number;
    onOpen: () => void;
  },
  "crate"
>;

function CrateCard({ data }: NodeProps<CrateNode>) {
  const { t } = useTranslation();
  return (
    <Card
      className="architecture-card !w-[260px] !rounded-xl !p-0"
      style={{ "--architecture-color": data.color } as CSSProperties}
    >
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <button
        type="button"
        className="nodrag nopan flex min-h-24 w-full flex-col gap-3 rounded-xl p-4 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
        onClick={data.onOpen}
        aria-pressed={data.focused}
        title={data.manifest}
      >
        <span className="flex items-start gap-2 text-sm font-semibold">
          <FiPackage className="mt-0.5 shrink-0" style={{ color: data.color }} />
          <span className="break-all">{data.name}</span>
        </span>
        <span className="flex gap-2 text-[10px] text-muted-foreground">
          <Badge variant="outline">
            {t("architectureModal.crates.incoming", { count: data.incoming })}
          </Badge>
          <Badge variant="outline">
            {t("architectureModal.crates.outgoing", { count: data.outgoing })}
          </Badge>
        </span>
      </button>
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </Card>
  );
}
const nodeTypes = { crate: CrateCard };
export const crateCount = crates.length;

// Consumers occupy the left layers; dependencies advance to the right.
const dependencies = new Map(
  crates.map((item) => [item.name, [...new Set(item.dependencies.map((dep) => dep.name))]]),
);
const incoming = new Map(
  crates.map((item) => [
    item.name,
    crates
      .filter((other) => dependencies.get(other.name)!.includes(item.name))
      .map((other) => other.name),
  ]),
);
const remaining = new Map([...incoming].map(([name, parents]) => [name, parents.length]));
const levels = new Map(crates.map((item) => [item.name, 0]));
const queue = crates.filter((item) => !remaining.get(item.name)).map((item) => item.name);
for (const name of queue) {
  for (const dependency of dependencies.get(name)!) {
    levels.set(dependency, Math.max(levels.get(dependency)!, levels.get(name)! + 1));
    remaining.set(dependency, remaining.get(dependency)! - 1);
    if (remaining.get(dependency) === 0) queue.push(dependency);
  }
}
const positions = new Map<string, { x: number; y: number }>();
for (let level = 0; level <= Math.max(...levels.values()); level++) {
  const layer = crates.filter((item) => levels.get(item.name) === level);
  const parentCenter = (name: string) => {
    const parents = incoming
      .get(name)!
      .flatMap((parent) => (positions.has(parent) ? [positions.get(parent)!.y] : []));
    return parents.length ? parents.reduce((sum, y) => sum + y, 0) / parents.length : 0;
  };
  layer.sort((a, b) => parentCenter(a.name) - parentCenter(b.name) || a.name.localeCompare(b.name));
  layer.forEach((item, index) =>
    positions.set(item.name, { x: level * 420, y: (index - (layer.length - 1) / 2) * 150 }),
  );
}

export function CrateDependencies() {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const requested = new URLSearchParams(location.search).get("crate");
  const selected = crates.some((item) => item.name === requested) ? requested : null;
  const { nodes, edges } = useMemo(() => {
    const open = (name: string) => {
      const params = new URLSearchParams(location.search);
      if (name) params.set("crate", name);
      else params.delete("crate");
      void navigate({ ...location, search: `?${params}` });
    };
    const nodes: CrateNode[] = crates.map((item) => ({
      id: item.name,
      type: "crate",
      style: { pointerEvents: "auto" },
      position: positions.get(item.name)!,
      data: {
        name: item.name,
        manifest: item.manifest,
        focused: item.name === selected,
        color:
          item.name === selected
            ? "var(--primary)"
            : `var(--chart-${(levels.get(item.name)! % 4) + 1})`,
        incoming: incoming.get(item.name)!.length,
        outgoing: dependencies.get(item.name)!.length,
        onOpen: () => open(item.name === selected ? "" : item.name),
      },
    }));
    const edges: Edge[] = [];
    for (const item of crates) {
      for (const name of new Set(item.dependencies.map((dep) => dep.name))) {
        const highlighted = item.name === selected || name === selected;
        const color = highlighted ? "var(--primary)" : "var(--muted-foreground)";
        const declarations = item.dependencies.filter((dep) => dep.name === name);
        const conditional = declarations.every(
          (dep) => dep.optional || dep.target || dep.kind === "build",
        );
        edges.push({
          id: `${item.name}:${name}`,
          source: item.name,
          target: name,
          type: "smoothstep",
          animated: !selected || highlighted,
          zIndex: highlighted ? 1 : 0,
          markerEnd: { type: MarkerType.ArrowClosed, color },
          style: {
            stroke: color,
            strokeWidth: highlighted ? 2.5 : 1.2,
            opacity: selected && !highlighted ? 0.2 : 0.7,
            strokeDasharray: conditional ? "5 5" : undefined,
          },
          label: conditional && highlighted ? t("architectureModal.crates.conditional") : undefined,
        });
      }
    }
    return { nodes, edges };
  }, [selected, location, navigate, t]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-border/40 px-5 py-2">
        <p className="text-[11px] text-muted-foreground">{t("architectureModal.crates.legend")}</p>
      </div>
      <div className="min-h-0 flex-1">
        <ReactFlow<CrateNode>
          className="architecture-flow"
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.15, maxZoom: 1 }}
          minZoom={0.05}
          nodesDraggable={false}
          nodesConnectable={false}
          edgesFocusable={false}
          elementsSelectable={false}
          deleteKeyCode={null}
          proOptions={{ hideAttribution: true }}
          aria-label={t("architectureModal.dependenciesPage")}
        >
          <Background gap={24} size={1} />
          <Controls showInteractive={false} />
          <MiniMap<CrateNode>
            nodeColor={(node) => node.data.color}
            pannable
            zoomable
            ariaLabel={t("architectureModal.minimap")}
          />
        </ReactFlow>
      </div>
    </div>
  );
}
