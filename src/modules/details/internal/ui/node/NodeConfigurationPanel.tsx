import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { applyGraphDraftMutation } from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphDraftEditingLocked } from "@/features/application/graphDraft/useGraphDraftEditingLocked";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { useGraphRead } from "@/features/core/graph/read";
import type { ParameterEditorDto } from "@/shared/types/domain/editorProjection";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { DetailForm } from "../shared/DetailForm";
import { ParameterValueEditor } from "./parameterEditors/NodeParameterEditor";
import { graphDraftMutationMessageKey, graphDraftMutationSucceeded } from "./nodeMutationFeedback";

function ConfigurationEditor({
  graphPath,
  nodeId,
  parameter,
}: {
  graphPath: string;
  nodeId: string;
  parameter: ParameterEditorDto;
}) {
  const { t, i18n } = useTranslation();
  const locked = useGraphDraftEditingLocked(graphPath);
  const [pending, setPending] = useState(false);
  const pendingRef = useRef(false);
  const [error, setError] = useState<string | null>(null);

  const update = async (values: Record<string, unknown>): Promise<boolean> => {
    if (pendingRef.current || locked) return false;
    pendingRef.current = true;
    setPending(true);
    setError(null);
    try {
      const result = await applyGraphDraftMutation({
        graphPath,
        locale: i18n.resolvedLanguage ?? "en-US",
        mutation: { type: "setConfiguration", payload: { nodeId, key: parameter.key, values } },
      });
      const errorKey = graphDraftMutationMessageKey(result, "detail.configuration.updateFailed");
      if (errorKey) setError(t(errorKey));
      return graphDraftMutationSucceeded(result);
    } catch (error) {
      setError(formatInlineUserError(error, t));
      return false;
    } finally {
      pendingRef.current = false;
      setPending(false);
    }
  };

  if (parameter.configuration?.kind !== "configuration") return null;
  return (
    <div
      tabIndex={-1}
      data-graph-path={graphPath}
      data-node-id={nodeId}
      data-graph-parameter-key={parameter.key}
    >
      <DetailCollapsibleSection title={parameter.display.title} defaultOpen>
        <DetailForm>
          {parameter.configuration.fields.map((field) => (
            <ParameterValueEditor
              key={field.key}
              parameter={field}
              pending={pending || locked}
              errors={[]}
              onCommit={(value, callbacks) => {
                if (Object.is(value, field.value)) {
                  callbacks?.onResolved?.();
                  return;
                }
                void update({ [field.key]: value }).then((ok) => {
                  if (ok) callbacks?.onResolved?.();
                  else callbacks?.onRejected?.();
                });
              }}
              formatFallback={(value) => (value == null ? "—" : String(value))}
            />
          ))}
          {error ? (
            <div role="alert" className="px-1 py-2 text-xs text-destructive">
              {error}
            </div>
          ) : null}
        </DetailForm>
      </DetailCollapsibleSection>
    </div>
  );
}

export function NodeConfigurationPanel({
  graphPath,
  nodeId,
}: {
  graphPath: string;
  nodeId: string;
}) {
  const node = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]?.nodes[nodeId]);
  return (
    <>
      {node?.parameterEditors
        .filter((parameter) => parameter.editor === "configuration")
        .map((parameter) => (
          <ConfigurationEditor
            key={`${graphPath}:${nodeId}:${parameter.key}`}
            graphPath={graphPath}
            nodeId={nodeId}
            parameter={structuredClone(parameter) as ParameterEditorDto}
          />
        ))}
    </>
  );
}
