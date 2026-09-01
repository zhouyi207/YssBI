import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscError } from "react-icons/vsc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { DataType } from "@/shared/types/domain/dataType";
import type { DataValue } from "@/shared/types/domain/dataValue";
import {
  dataValueToEditableJson,
  isJsonEditableVariableType,
  parseArrayValueFromJson,
  parseDataFrameValueFromJson,
  parseDataSeriesValueFromJson,
  parseObjectValueFromJson,
} from "./variableValueUtils";

interface VariableValueEditorModalProps {
  open: boolean;
  onClose: () => void;
  dataType: DataType;
  dataValue: DataValue;
  onSave: (value: DataValue) => void;
}

const jsonTextareaClass =
  "min-h-[220px] w-full resize-y rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30";

export function VariableValueEditorModal({
  open,
  onClose,
  dataType,
  dataValue,
  onSave,
}: VariableValueEditorModalProps) {
  const { t } = useTranslation();
  const [jsonDraft, setJsonDraft] = useState("");
  const [jsonError, setJsonError] = useState<string | null>(null);
  const jsonErrorId = useId();

  useEffect(() => {
    if (!open || !isJsonEditableVariableType(dataType)) return;
    setJsonDraft(dataValueToEditableJson(dataType, dataValue));
    setJsonError(null);
  }, [open, dataType, dataValue]);

  const handleClear = () => {
    setJsonError(null);
    onSave({ kind: "Null" });
    onClose();
  };

  const handleSave = () => {
    let result: { ok: true; value: DataValue } | { ok: false; error: string };

    switch (dataType.kind) {
      case "Array":
        result = parseArrayValueFromJson(jsonDraft, dataType.inner ?? { kind: "Any" });
        break;
      case "Object":
        result = parseObjectValueFromJson(jsonDraft);
        break;
      case "DataFrame":
        result = parseDataFrameValueFromJson(jsonDraft);
        break;
      case "DataSeries":
        result = parseDataSeriesValueFromJson(jsonDraft);
        break;
      default:
        return;
    }

    if (!result.ok) {
      setJsonError(t(`detail.variableValue.errors.${result.error}`));
      return;
    }
    setJsonError(null);
    onSave(result.value);
    onClose();
  };

  if (!isJsonEditableVariableType(dataType)) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="flex max-h-[85vh] max-w-[560px] flex-col overflow-hidden p-0">
        <DialogHeader className="border-b border-border bg-muted/20 px-6 py-4">
          <DialogTitle>{t(`detail.variableValue.title.${dataType.kind}`)}</DialogTitle>
          <DialogDescription>
            {t(`detail.variableValue.description.${dataType.kind}`)}
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 flex-1 px-6 py-5">
          <div className="flex min-h-0 flex-1 flex-col gap-2">
            <Label htmlFor="variable-value-json">{t("detail.variableValue.jsonLabel")}</Label>
            <ScrollArea className="min-h-0 flex-1">
              <textarea
                id="variable-value-json"
                className={jsonTextareaClass}
                value={jsonDraft}
                spellCheck={false}
                aria-invalid={Boolean(jsonError)}
                aria-describedby={jsonError ? jsonErrorId : undefined}
                onChange={(event) => {
                  setJsonDraft(event.target.value);
                  setJsonError(null);
                }}
              />
            </ScrollArea>
            {jsonError ? (
              <Alert id={jsonErrorId} variant="destructive">
                <VscError aria-hidden="true" />
                <AlertDescription className="text-destructive">{jsonError}</AlertDescription>
              </Alert>
            ) : null}
          </div>
        </div>

        <DialogFooter className="border-t border-border px-6 py-4">
          <Button type="button" variant="ghost" size="lg" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button type="button" variant="outline" size="lg" onClick={handleClear}>
            {t("detail.variableValue.clear")}
          </Button>
          <Button type="button" size="lg" onClick={handleSave}>
            {t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
