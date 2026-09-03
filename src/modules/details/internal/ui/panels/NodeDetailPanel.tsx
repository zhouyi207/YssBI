import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import type { GraphEntitiesState } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphRead } from "@/features/core/graph/read";
import { derivePinConnectionView } from "@/features/core/dataStore/pinLinks";
import { formatDiagnosticLocationLabel } from "@/features/domain/graphDiagnostics/nodeDiagnostics";

import type { PinData, PinView } from "@/features/domain/editorProjection/graphRuntimeTypes";

import { DetailPanelShell } from "../shared/DetailPanelShell";
import { NodeDocumentationPanel } from "../node/NodeDocumentationPanel";
import { NodePinInterfacePanel } from "../node/NodePinInterfacePanel";
import type { NodePinViewModel } from "../node/NodePinViewModel";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { DetailBadge, DetailText } from "../shared/DetailText";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";

const EMPTY_PINS: PinData[] = [];
const EMPTY_PIN_CONNECTIONS: string[][] = [];

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

export function selectNodeDetailNode(state: GraphEntitiesState, graphPath: string, nodeId: string) {
  return state.graphEntities[graphPath]?.nodes[nodeId];
}

interface NodeDetailPanelProps {
  graphPath: string;
  nodeId: string;
}

export function NodeDetailPanel({ graphPath, nodeId }: NodeDetailPanelProps) {
  const { t } = useTranslation();
  const node = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]?.nodes[nodeId]);
  const graphBucket = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]);
  const { catalog } = useLocalizedNodeCatalog(Boolean(node));
  const pinObjs = useGraphRead((snapshot) => {
    const bucket = snapshot.graphEntities[graphPath];
    const pinIds = bucket?.nodes[nodeId]?.pinIds;
    if (!bucket || !pinIds?.length) return EMPTY_PINS;
    return pinIds
      .map((pinId) => bucket.pins[pinId])
      .filter(isPresent)
      .map((pin) => structuredClone(pin) as PinData);
  });
  const pinConns = useGraphRead((snapshot) => {
    const bucket = snapshot.graphEntities[graphPath];
    const pinIds = bucket?.nodes[nodeId]?.pinIds;
    if (!bucket || !pinIds?.length) return EMPTY_PIN_CONNECTIONS;
    return pinIds.map((pinId) => [...(bucket.pinConnections[pinId] ?? [])]);
  });

  const pins = useMemo<PinView[]>(
    () =>
      pinObjs.map((pin, index) => ({
        ...pin,
        ...derivePinConnectionView(pinConns[index]),
      })),
    [pinObjs, pinConns],
  );

  const pinSpecs = useMemo(() => {
    const toViewModel = (pin: PinView): NodePinViewModel => ({
      id: pin.id,
      name: pin.display.instanceLabel ?? pin.display.label,
      direction: pin.direction,
    });
    return {
      inputs: pins.filter((pin) => pin.direction === "input").map(toViewModel),
      outputs: pins.filter((pin) => pin.direction === "output").map(toViewModel),
    };
  }, [pins]);

  if (!node) {
    return (
      <DetailPanelShell>
        <DetailText as="div" tone="muted" className="p-4">
          {t("detail.nodeNotFound")}
        </DetailText>
      </DetailPanelShell>
    );
  }

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

      <DetailCollapsibleSection title={t("detail.sections.capabilities")}>
        <div className="flex flex-wrap gap-1.5 px-1 py-2">
          {Object.entries(node.capabilities)
            .filter(([, enabled]) => enabled)
            .map(([capability]) => (
              <DetailBadge key={capability}>{capability}</DetailBadge>
            ))}
        </div>
      </DetailCollapsibleSection>
      {node.diagnostics.length > 0 && (
        <DetailCollapsibleSection title={t("detail.sections.diagnostics")} defaultOpen>
          <div className="space-y-2 px-1 py-2">
            {node.diagnostics.map((diagnostic, index) => {
              const locationLabel = formatDiagnosticLocationLabel(
                diagnostic.location,
                graphBucket,
                nodeId,
              );
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
                      {diagnostic.message}
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
}
