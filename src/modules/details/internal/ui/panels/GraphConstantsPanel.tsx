import { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscAdd, VscRemove, VscReferences } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  createGraphConstant,
  deleteGraphConstant,
  editableConstantValue,
  insertConstantReference,
  updateGraphConstant,
  useGraphConstants,
} from "@/features/application/graphDraft/graphConstantActions";
import type { ApplyGraphDraftMutationOutcome } from "@/features/application/graphDraft/graphDraftCoordinator";
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
  const run = async (operation: () => Promise<ApplyGraphDraftMutationOutcome>) => {
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
      <fieldset disabled={busy || saving || !loaded} className="min-w-0 space-y-2">
        <div className="flex justify-end px-3">
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
          <div
            key={constant.id}
            data-constant-id={constant.id}
            className="border-t border-border/50"
          >
            <ConstantValueFields
              constant={constant}
              onUpdate={(patch) =>
                void run(() => updateGraphConstant(graphPath, constant.id, patch))
              }
            />
            <div className="flex justify-end gap-1 px-3 pb-2">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void run(() => insertConstantReference(graphPath, constant.id))}
              >
                <VscReferences aria-hidden />
                {t("detail.constants.insertReference")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void run(() => deleteGraphConstant(graphPath, constant.id))}
                aria-label={t("detail.constants.remove", { name: constant.name })}
              >
                <VscRemove aria-hidden />
              </Button>
            </div>
          </div>
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
