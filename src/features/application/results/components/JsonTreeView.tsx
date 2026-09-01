import { useCallback, useMemo, useState } from "react";
import { VscChevronDown, VscChevronRight } from "react-icons/vsc";

const DEFAULT_EXPAND_DEPTH = 2;

function isExpandable(value: unknown): value is Record<string, unknown> | unknown[] {
  return value !== null && typeof value === "object";
}

function formatLeaf(value: unknown): string {
  if (value === null) return "null";
  if (value === undefined) return "undefined";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

function leafTypeLabel(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return `Array(${value.length})`;
  if (typeof value === "object") return "Object";
  return typeof value;
}

interface JsonTreeNodeProps {
  label: string;
  value: unknown;
  depth: number;
  path: string;
  defaultExpandDepth: number;
}

function JsonTreeNode({ label, value, depth, path, defaultExpandDepth }: JsonTreeNodeProps) {
  const expandable = isExpandable(value);
  const [expanded, setExpanded] = useState(depth < defaultExpandDepth);

  const toggle = useCallback(() => {
    if (expandable) setExpanded((prev) => !prev);
  }, [expandable]);

  if (!expandable) {
    return (
      <div className="flex min-w-0 items-start gap-1 py-0.5" style={{ paddingLeft: depth * 12 }}>
        <span className="w-4 shrink-0" />
        <span className="shrink-0 font-medium text-muted-foreground">{label}:</span>
        <span className="min-w-0 break-all font-mono text-[13px] text-[var(--accent-color)]">
          {formatLeaf(value)}
        </span>
      </div>
    );
  }

  const entries = Array.isArray(value)
    ? value.map((item, index) => ({ key: String(index), child: item }))
    : Object.entries(value as Record<string, unknown>).map(([key, child]) => ({ key, child }));

  const summary = Array.isArray(value) ? `Array(${value.length})` : `Object{${entries.length}}`;

  return (
    <div className="min-w-0">
      <button
        type="button"
        className="flex w-full min-w-0 items-center gap-1 rounded py-0.5 text-left hover:bg-muted/50"
        style={{ paddingLeft: depth * 12 }}
        onClick={toggle}
      >
        <span className="flex w-4 shrink-0 items-center justify-center text-muted-foreground">
          {expanded ? <VscChevronDown size={14} /> : <VscChevronRight size={14} />}
        </span>
        <span className="shrink-0 font-medium text-foreground">{label}</span>
        <span className="shrink-0 text-xs text-muted-foreground">({summary})</span>
      </button>
      {expanded ? (
        <div>
          {entries.length === 0 ? (
            <div
              className="py-0.5 text-xs text-muted-foreground"
              style={{ paddingLeft: depth * 12 + 20 }}
            >
              empty
            </div>
          ) : (
            entries.map(({ key, child }) => (
              <JsonTreeNode
                key={`${path}.${key}`}
                label={key}
                value={child}
                depth={depth + 1}
                path={`${path}.${key}`}
                defaultExpandDepth={defaultExpandDepth}
              />
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}

export interface JsonTreeViewProps {
  value: unknown;
  defaultExpandDepth?: number;
  rootLabel?: string;
}

export function JsonTreeView({
  value,
  defaultExpandDepth = DEFAULT_EXPAND_DEPTH,
  rootLabel = "value",
}: JsonTreeViewProps) {
  const rootSummary = useMemo(() => leafTypeLabel(value), [value]);

  if (!isExpandable(value)) {
    return (
      <div className="rounded-lg border border-border bg-card p-3 font-mono text-sm">
        <div className="mb-1 text-xs text-muted-foreground">{rootSummary}</div>
        <div className="break-all text-[var(--accent-color)]">{formatLeaf(value)}</div>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-card p-2 font-mono text-sm">
      <JsonTreeNode
        label={rootLabel}
        value={value}
        depth={0}
        path="root"
        defaultExpandDepth={defaultExpandDepth}
      />
    </div>
  );
}
