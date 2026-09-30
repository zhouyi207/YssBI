import { useShallow } from "zustand/react/shallow";
import { Fragment, memo, useMemo, type ComponentProps } from "react";
import { useTranslation } from "react-i18next";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { useGraphRead } from "@/features/core/graph/read";
import {
  formatDiagnosticLocationLabel,
  formatGraphDiagnostic,
} from "@/features/domain/graphDiagnostics/nodeDiagnostics";

import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { NodeParameterEditor } from "../node/parameterEditors/NodeParameterEditor";

import { DetailPanelShell } from "../shared/DetailPanelShell";
import { NodeDocumentationPanel } from "../node/NodeDocumentationPanel";
import { NodePinInterfacePanel } from "../node/NodePinInterfacePanel";
import type { NodePinViewModel } from "../node/NodePinViewModel";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { DetailBadge, DetailText } from "../shared/DetailText";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";

const EMPTY_PINS: PinData[] = [];

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

interface NodeDetailPanelProps {
  graphPath: string;
  nodeId: string;
}

function formatParameterValue(value: unknown): string {
  return value == null ? "—" : typeof value === "string" ? value : JSON.stringify(value);
}

const NodeParameterSection = memo(function NodeParameterSection({
  graphPath,
  nodeId,
  locale,
  parameter,
}: Omit<ComponentProps<typeof NodeParameterEditor>, "diagnostics" | "formatFallback">) {
  const diagnostics = useGraphRead(
    useShallow((snapshot) =>
      (snapshot.graphEntities[graphPath]?.nodes[nodeId]?.diagnostics ?? []).filter(
        (diagnostic) =>
          diagnostic.location.kind === "parameter" &&
          diagnostic.location.nodeId === nodeId &&
          diagnostic.location.key === parameter.key,
      ),
    ),
  );
  return (
    <div
      tabIndex={-1}
      data-graph-path={graphPath}
      data-node-id={nodeId}
      data-graph-parameter-key={parameter.key}
    >
      <DetailCollapsibleSection title={parameter.display.title} defaultOpen>
        <NodeParameterEditor
          graphPath={graphPath}
          nodeId={nodeId}
          locale={locale}
          parameter={parameter}
          diagnostics={diagnostics}
          formatFallback={formatParameterValue}
        />
      </DetailCollapsibleSection>
    </div>
  );
});

export const NodeDetailPanel = memo(function NodeDetailPanel({
  graphPath,
  nodeId,
}: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const node = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]?.nodes[nodeId]);
  const diagnosticLabels = useGraphRead(
    useShallow((snapshot) => {
      const bucket = snapshot.graphEntities[graphPath];
      return (bucket?.nodes[nodeId]?.diagnostics ?? []).map((diagnostic) =>
        formatDiagnosticLocationLabel(diagnostic.location, bucket, nodeId),
      );
    }),
  );
  const { catalog } = useLocalizedNodeCatalog(Boolean(node));
  const pins = useGraphRead(
    useShallow((snapshot) => {
      const bucket = snapshot.graphEntities[graphPath];
      const pinIds = bucket?.nodes[nodeId]?.pinIds;
      return bucket && pinIds?.length
        ? pinIds.map((pinId) => bucket.pins[pinId]).filter(isPresent)
        : EMPTY_PINS;
    }),
  );

  const pinSpecs = useMemo(() => {
    const toViewModel = (pin: (typeof pins)[number]): NodePinViewModel => ({
      id: pin.id,
      name: pin.display.instanceLabel ?? pin.display.label,
      direction: pin.direction,
    });
    return {
      inputs: pins.filter((pin) => pin.direction === "input").map(toViewModel),
      outputs: pins.filter((pin) => pin.direction === "output").map(toViewModel),
    };
  }, [pins]);

  if (!node) return null;

  const catalogItem = catalog?.items.find((item) => item.nodeTypeId === node.nodeType);
  const documentation = catalogItem?.documentation;

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField
          label={t("detail.fields.name")}
          tone="body"
          valueClassName="min-w-0"
          className="min-w-0 truncate font-medium"
        >
          {node.display.title}
        </DetailReadonlyField>
      </DetailForm>

      {node.parameterGroups.map((group) => (
        <Fragment key={`${graphPath}:${nodeId}:${group.key}`}>
          {group.display.description && (
            <DetailText as="div" tone="muted" className="px-3 py-2 text-xs">
              {group.display.description}
            </DetailText>
          )}
          {group.parameters.map((parameter) => (
            <NodeParameterSection
              key={parameter.key}
              graphPath={graphPath}
              nodeId={nodeId}
              locale={i18n?.resolvedLanguage ?? "en-US"}
              parameter={parameter}
            />
          ))}
        </Fragment>
      ))}

      {node.diagnostics.length > 0 && (
        <DetailCollapsibleSection title={t("detail.sections.diagnostics")} defaultOpen>
          <div className="space-y-2 px-1 py-2">
            {node.diagnostics.map((diagnostic, index) => {
              const locationLabel = diagnosticLabels[index];
              const nodeTitle = node.display.title;
              return (
                <div key={`${diagnostic.code}-${index}`} className="flex items-start gap-2">
                  <DetailBadge>{diagnostic.severity}</DetailBadge>
                  <div className="min-w-0">
                    {locationLabel && locationLabel !== nodeTitle ? (
                      <DetailText as="div" className="font-medium" tone="muted">
                        {locationLabel}
                      </DetailText>
                    ) : null}
                    <DetailText as="span" tone="muted">
                      {formatGraphDiagnostic(diagnostic, i18n?.resolvedLanguage)}
                    </DetailText>
                  </div>
                </div>
              );
            })}
          </div>
        </DetailCollapsibleSection>
      )}
      <NodePinInterfacePanel
        graphPath={graphPath}
        nodeId={nodeId}
        inputs={pinSpecs.inputs}
        outputs={pinSpecs.outputs}
        portInstanceAdditions={node.portInstanceAdditions}
      />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
});
