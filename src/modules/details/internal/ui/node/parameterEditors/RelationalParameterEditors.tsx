import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useId, useRef, useState, type FocusEvent } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type {
  FilterLiteralDto,
  FilterOperatorDto,
  FilterPredicateDto,
  ParameterEditorSpecDto,
} from "@/shared/types/domain/editorProjection";

interface EditorProps<TEditor, TValue> {
  editor: TEditor;
  errors: readonly string[];
  disabled?: boolean;
  onCommit(value: TValue, callbacks?: { onRejected?(): void }): void | Promise<void>;
}

type ProjectEditor = DeepReadonly<Extract<ParameterEditorSpecDto, { kind: "projectColumns" }>>;
type FilterEditor = DeepReadonly<Extract<ParameterEditorSpecDto, { kind: "filterPredicate" }>>;

function EditorMessages({
  unavailable,
  errors,
}: {
  unavailable?: string | null;
  errors: readonly string[];
}) {
  return (
    <div className="space-y-1 text-xs" aria-live="polite">
      {unavailable && <p className="text-muted-foreground">{unavailable}</p>}
      {errors.map((error) => (
        <p key={error} className="text-destructive">
          {error}
        </p>
      ))}
    </div>
  );
}

export function ProjectColumnsEditor({
  editor,
  errors,
  disabled = false,
  onCommit,
}: EditorProps<ProjectEditor, string[]>) {
  const { t } = useTranslation();
  const id = useId();
  const selected = editor.value;

  if (!editor.available) {
    return <EditorMessages unavailable={editor.unavailableReason} errors={errors} />;
  }

  const toggle = (name: string, checked: boolean) => {
    if (disabled) return;
    const next = checked ? [...selected, name] : selected.filter((column) => column !== name);
    if (editor.allowEmpty || next.length > 0) void onCommit(next);
  };
  const move = (name: string, offset: -1 | 1) => {
    const from = selected.indexOf(name);
    const to = from + offset;
    if (disabled || from < 0 || to < 0 || to >= selected.length) return;
    const reordered = [...selected];
    [reordered[from], reordered[to]] = [reordered[to], reordered[from]];
    void onCommit(reordered);
  };

  return (
    <div className="space-y-3" aria-busy={disabled}>
      <div className="space-y-2">
        {editor.options.map((option, index) => {
          const position = selected.indexOf(option.name);
          const checked = position >= 0;
          const required = !editor.allowEmpty && selected.length === 1 && checked;
          return (
            <div key={option.name} className="flex h-5 items-center gap-2">
              <Checkbox
                id={`${id}-${index}`}
                aria-label={t("detail.parameterEditor.selectColumn", { column: option.name })}
                checked={checked}
                className={disabled ? "cursor-wait" : undefined}
                disabled={required}
                aria-disabled={disabled || required}
                onCheckedChange={(checked) => toggle(option.name, checked === true)}
              />
              <Label
                htmlFor={`${id}-${index}`}
                className={cn("min-w-0 flex-1", disabled && "cursor-wait")}
              >
                <span className="truncate">{option.name}</span>
                <span className="ml-2 text-xs text-muted-foreground">{option.dataType}</span>
              </Label>
              <div
                aria-hidden={!checked}
                className={cn("flex w-20 shrink-0 items-center gap-0.5", !checked && "invisible")}
              >
                <span className="mr-1 min-w-0 flex-1 text-right text-xs tabular-nums text-muted-foreground">
                  {checked ? position + 1 : ""}
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t("detail.parameterEditor.moveColumnUp", { column: option.name })}
                  disabled={position <= 0}
                  aria-disabled={disabled || position <= 0}
                  className={disabled ? "cursor-wait" : undefined}
                  onClick={() => move(option.name, -1)}
                >
                  ↑
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t("detail.parameterEditor.moveColumnDown", { column: option.name })}
                  disabled={!checked || position === selected.length - 1}
                  aria-disabled={disabled || !checked || position === selected.length - 1}
                  className={disabled ? "cursor-wait" : undefined}
                  onClick={() => move(option.name, 1)}
                >
                  ↓
                </Button>
              </div>
            </div>
          );
        })}
      </div>
      <EditorMessages errors={errors} />
    </div>
  );
}

function defaultLiteralType(
  column: FilterEditor["columns"][number] | undefined,
): FilterLiteralDto["type"] {
  return column?.literalTypes[0] ?? "string";
}

function issuedOperator(
  column: FilterEditor["columns"][number] | undefined,
  requested?: FilterOperatorDto,
): FilterOperatorDto | undefined {
  return requested && column?.operators.includes(requested) ? requested : column?.operators[0];
}

function operatorNeedsValue(operator: FilterOperatorDto | undefined): boolean {
  return operator !== undefined && operator !== "isNull" && operator !== "isNotNull";
}

function literalValue(
  literal: FilterLiteralDto | undefined,
  type: FilterLiteralDto["type"],
): string {
  if (!literal) return type === "boolean" ? "false" : "";
  return literal.type === "boolean" ? String(literal.value) : literal.value;
}

interface PredicateDraft {
  column: string;
  operator: FilterOperatorDto | undefined;
  literalType: FilterLiteralDto["type"];
  value: string;
}

function projectedPredicate(editor: FilterEditor): PredicateDraft {
  const column = editor.value?.column ?? "";
  const option = editor.columns.find((candidate) => candidate.name === column);
  const literalType = editor.value?.value?.type ?? defaultLiteralType(option);
  return {
    column,
    operator: issuedOperator(option, editor.value?.operator),
    literalType,
    value: literalValue(editor.value?.value, literalType),
  };
}

export function FilterPredicateEditor({
  editor,
  errors,
  disabled = false,
  onCommit,
}: EditorProps<FilterEditor, FilterPredicateDto>) {
  const { t } = useTranslation();
  const id = useId();
  const latestEditor = useRef(editor);
  latestEditor.current = editor;
  const [projection, setProjection] = useState(editor);
  const [draft, setDraft] = useState(() => projectedPredicate(editor));
  if (projection !== editor) {
    setProjection(editor);
    setDraft(projectedPredicate(editor));
  }
  const { column, operator, literalType, value } = draft;
  const selectedColumn = editor.columns.find((option) => option.name === column);

  if (!editor.available) {
    return <EditorMessages unavailable={editor.unavailableReason} errors={errors} />;
  }

  const reset = () => setDraft(projectedPredicate(latestEditor.current));
  const submit = (next: PredicateDraft) => {
    const option = editor.columns.find((candidate) => candidate.name === next.column);
    if (disabled || !option || !next.operator || !option.operators.includes(next.operator)) return;
    const predicate: FilterPredicateDto = { column: next.column, operator: next.operator };
    if (operatorNeedsValue(next.operator)) {
      if (!option.literalTypes.includes(next.literalType)) return;
      if (next.literalType !== "string" && next.value.length === 0) return;
      predicate.value =
        next.literalType === "boolean"
          ? { type: "boolean", value: next.value === "true" }
          : { type: next.literalType, value: next.value };
    }
    if (JSON.stringify(predicate) !== JSON.stringify(editor.value)) {
      void onCommit(predicate, { onRejected: reset });
    }
  };
  const change = (next: PredicateDraft) => {
    setDraft(next);
    submit(next);
  };
  const chooseColumn = (name: string) => {
    const option = editor.columns.find((candidate) => candidate.name === name);
    const nextType = option?.literalTypes.includes(literalType)
      ? literalType
      : defaultLiteralType(option);
    change({
      column: name,
      operator: issuedOperator(option, operator),
      literalType: nextType,
      value: nextType === literalType ? value : literalValue(undefined, nextType),
    });
  };
  const blur = (event: FocusEvent<HTMLDivElement>) => {
    const target = event.relatedTarget;
    if (target instanceof Node && event.currentTarget.contains(target)) return;
    // Radix select content is portaled outside this editor's DOM subtree.
    if (
      target instanceof Element &&
      target.closest("[data-parameter-editor]")?.getAttribute("data-parameter-editor") === id
    )
      return;
    submit(draft);
  };

  return (
    <div className="space-y-3" data-parameter-editor={id} onBlur={blur}>
      <div className="space-y-1.5">
        <Label>{t("detail.parameterEditor.column")}</Label>
        <Select value={column} onValueChange={chooseColumn} disabled={disabled}>
          <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateColumn")}>
            <SelectValue placeholder={t("detail.parameterEditor.column")} />
          </SelectTrigger>
          <SelectContent data-parameter-editor={id}>
            {editor.columns.map((option) => (
              <SelectItem key={option.name} value={option.name}>
                {option.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-1.5">
        <Label>{t("detail.parameterEditor.operator")}</Label>
        <Select
          key={column}
          value={operator ?? ""}
          onValueChange={(value) => change({ ...draft, operator: value as FilterOperatorDto })}
          disabled={disabled || !selectedColumn || selectedColumn.operators.length === 0}
        >
          <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateOperator")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent data-parameter-editor={id}>
            {selectedColumn?.operators.map((option) => (
              <SelectItem key={option} value={option}>
                {option}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {operatorNeedsValue(operator) && (selectedColumn?.literalTypes.length ?? 0) > 1 && (
        <div className="space-y-1.5">
          <Label>{t("detail.parameterEditor.valueType")}</Label>
          <Select
            value={literalType}
            onValueChange={(type) => {
              const nextType = type as FilterLiteralDto["type"];
              change({ ...draft, literalType: nextType, value: literalValue(undefined, nextType) });
            }}
            disabled={disabled}
          >
            <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateValueType")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent data-parameter-editor={id}>
              {selectedColumn?.literalTypes.map((type) => (
                <SelectItem key={type} value={type}>
                  {type}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      {operatorNeedsValue(operator) && literalType === "boolean" && (
        <Select
          value={value}
          onValueChange={(value) => change({ ...draft, value })}
          disabled={disabled}
        >
          <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateValue")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent data-parameter-editor={id}>
            <SelectItem value="true">true</SelectItem>
            <SelectItem value="false">false</SelectItem>
          </SelectContent>
        </Select>
      )}
      {operatorNeedsValue(operator) && literalType !== "boolean" && (
        <Input
          aria-label={t("detail.parameterEditor.predicateValue")}
          value={value}
          disabled={disabled}
          onChange={(event) => setDraft({ ...draft, value: event.target.value })}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "Enter") {
              event.preventDefault();
              event.currentTarget.blur();
            } else if (event.key === "Escape") {
              event.preventDefault();
              reset();
            }
          }}
        />
      )}
      <EditorMessages errors={errors} />
    </div>
  );
}
