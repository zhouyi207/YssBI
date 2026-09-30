import { useMemo, useRef, useState, type ComponentProps } from "react";
import { useDraggable } from "@dnd-kit/core";
import { useTranslation } from "react-i18next";
import { VscAdd, VscGripper, VscRemove } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  createGraphConstant,
  deleteGraphConstant,
  editableConstantValue,
  updateGraphConstant,
  useGraphConstants,
} from "@/features/application/graphEditing/graphConstantActions";
import type { ApplyGraphMutationOutcome } from "@/features/application/graphEditing/graphEditCoordinator";
import { DRAG_TYPES, type GraphConstantDragPayload } from "@/features/core/dnd";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { ConstantValueFields } from "./ConstantValueFields";

export function GraphConstantsPanel({ graphPath }: { graphPath: string }) {
  const { t } = useTranslation();
  const { constants, loaded, saving } = useGraphConstants(graphPath);
  const editableConstants = useMemo(
    () =>
      Object.values(constants).map((constant) => ({
        ...constant,
        dataValue: editableConstantValue(constant),
      })),
    [constants],
  );
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (operation: () => Promise<ApplyGraphMutationOutcome>) => {
    if (pending.current || saving || !loaded) return;
    pending.current = true;
    setBusy(true);
    setError(null);
    try {
      const result = await operation();
      if (result.status !== "applied" && result.status !== "noop")
        setError(t("detail.constants.updateFailed"));
    } catch {
      setError(t("detail.constants.updateFailed"));
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };
  return (
    <DetailCollapsibleSection title={t("detail.constants.title")} defaultOpen>
      <fieldset disabled={busy || saving || !loaded} className="min-w-0 space-y-1">
        <div className="flex justify-end">
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              void run(() => createGraphConstant(graphPath, t("detail.constants.defaultName")))
            }
          >
            <VscAdd aria-hidden />
            {t("detail.constants.add")}
          </Button>
        </div>
        {editableConstants.map((constant) => (
          <GraphConstantRow
            key={constant.id}
            graphPath={graphPath}
            constant={constant}
            disabled={busy || saving || !loaded}
            onUpdate={(patch) => void run(() => updateGraphConstant(graphPath, constant.id, patch))}
            onRemove={() => void run(() => deleteGraphConstant(graphPath, constant.id))}
          />
        ))}
        {loaded && Object.keys(constants).length === 0 && (
          <p className="px-3 text-xs text-muted-foreground">{t("detail.constants.empty")}</p>
        )}
      </fieldset>
      {error && (
        <p role="alert" className="px-3 text-xs text-destructive">
          {error}
        </p>
      )}
    </DetailCollapsibleSection>
  );
}

function GraphConstantRow({
  graphPath,
  constant,
  disabled,
  onUpdate,
  onRemove,
}: {
  graphPath: string;
  constant: ComponentProps<typeof ConstantValueFields>["constant"];
  disabled: boolean;
  onUpdate: ComponentProps<typeof ConstantValueFields>["onUpdate"];
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, isDragging } = useDraggable({
    id: `graph-constant-${graphPath}-${constant.id}`,
    data: {
      type: DRAG_TYPES.GRAPH_CONSTANT,
      graphPath,
      constantId: constant.id,
      name: constant.name,
    } satisfies GraphConstantDragPayload,
    disabled,
  });
  const dragLabel = t("detail.constants.dragToCanvas", { name: constant.name });
  const removeLabel = t("detail.constants.remove", { name: constant.name });

  return (
    <div
      ref={setNodeRef}
      data-constant-id={constant.id}
      data-dragging={isDragging || undefined}
      className="flex min-w-0 items-center gap-1 border-t border-border/50 py-1 data-[dragging=true]:opacity-50"
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            ref={setActivatorNodeRef}
            {...attributes}
            {...listeners}
            type="button"
            size="icon-xs"
            variant="ghost"
            className="touch-none cursor-grab text-muted-foreground active:cursor-grabbing"
            disabled={disabled}
            aria-label={dragLabel}
          >
            <VscGripper className="size-3.5" aria-hidden />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{dragLabel}</TooltipContent>
      </Tooltip>
      <ConstantValueFields constant={constant} onUpdate={onUpdate} />
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            onClick={onRemove}
            aria-label={removeLabel}
          >
            <VscRemove aria-hidden />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{removeLabel}</TooltipContent>
      </Tooltip>
    </div>
  );
}
