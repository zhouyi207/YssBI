import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscAdd, VscTrash } from "react-icons/vsc";

import { Button } from "@/components/ui/button";
import {
  addPortInstance,
  removePortInstance,
} from "@/features/application/editor/portInstanceActions";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { GraphDraftCommandResult } from "@/features/core/history/types";
import type { PortInstanceAdditionDto } from "@/shared/types/domain/editorProjection";
import { graphDraftMutationMessageKey } from "./nodeMutationFeedback";

interface PortInstanceControlProps {
  graphPath: string;
  disabled: boolean;
}

function PortInstanceMutationButton({
  label,
  testId,
  variant,
  icon,
  disabled,
  execute,
}: {
  label: string;
  testId: string;
  variant: "outline" | "destructive";
  icon: ReactNode;
  disabled: boolean;
  execute: () => Promise<GraphDraftCommandResult>;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  const handleClick = async () => {
    if (busy || disabled) return;
    setBusy(true);
    setErrorKey(null);
    try {
      const result = await execute();
      setErrorKey(graphDraftMutationMessageKey(result, "detail.nodeDoc.portInstanceFailed"));
    } catch {
      setErrorKey("detail.nodeDoc.portInstanceFailed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col items-end gap-1 px-1">
      <Button
        type="button"
        variant={variant}
        size="xs"
        disabled={busy || disabled}
        aria-label={label}
        data-testid={testId}
        onClick={() => void handleClick()}
      >
        {icon}
        {label}
      </Button>
      {errorKey ? (
        <div role="alert" className="text-[10px] text-destructive">
          {t(errorKey)}
        </div>
      ) : null}
    </div>
  );
}

export function AddNodePortInstanceButton({
  graphPath,
  nodeId,
  addition,
  disabled,
}: PortInstanceControlProps & {
  nodeId: string;
  addition: PortInstanceAdditionDto;
}) {
  const { t } = useTranslation();
  return (
    <PortInstanceMutationButton
      label={t("detail.nodeDoc.addPort", { name: addition.label })}
      testId={`add-port-instance-${addition.templateKey}`}
      variant="outline"
      icon={<VscAdd aria-hidden="true" data-icon="inline-start" />}
      disabled={disabled || !addition.canAdd}
      execute={() => addPortInstance(graphPath, nodeId, addition.templateKey)}
    />
  );
}

export function RemoveNodePortInstanceButton({
  graphPath,
  pin,
  disabled,
}: PortInstanceControlProps & { pin: PinData }) {
  const { t } = useTranslation();
  if (!pin.canRemove || pin.address.kind !== "instance") return null;
  const address = pin.address;
  return (
    <PortInstanceMutationButton
      label={t("detail.nodeDoc.removePort", { name: pin.name })}
      testId={`remove-port-instance-${pin.id}`}
      variant="destructive"
      icon={<VscTrash aria-hidden="true" data-icon="inline-start" />}
      disabled={disabled}
      execute={() => removePortInstance(graphPath, address)}
    />
  );
}
