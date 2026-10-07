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
import { FILTER_OPERATORS, FILTER_LITERAL_TYPES } from "@/shared/types/domain/editorProjection";

interface EditorProps<TEditor, TValue> {
  editor: TEditor;
  errors: readonly string[];
  disabled?: boolean;
  onCommit(
    value: TValue,
    callbacks?: { onResolved?(): void; onRejected?(): void },
  ): void | Promise<void>;
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
}: EditorProps<ProjectEditor, string[] | null>) {
  const { t } = useTranslation();
  const id = useId();
  const selected = editor.value;
  const [name, setName] = useState("");
  const eligibleNames = new Set(editor.options.map((option) => option.name));
  const options = editor.schemaKnown
    ? [
        ...editor.options,
        ...selected
          .filter((name) => !eligibleNames.has(name))
          .map((name) => ({ name, dataType: null })),
      ]
    : selected.map((name) => ({ name, dataType: null }));
  const commit = (next: string[]) =>
    onCommit(next.length === 0 && !editor.allowEmpty ? null : next);
  const add = () => {
    if (disabled || name.length === 0 || selected.includes(name)) return;
    if (editor.schemaKnown && !eligibleNames.has(name)) return;
    const submitted = name;
    void onCommit([...selected, name], {
      onResolved: () => setName((current) => (current === submitted ? "" : current)),
    });
  };

  const toggle = (name: string, checked: boolean) => {
    if (disabled) return;
    const next = checked ? [...selected, name] : selected.filter((column) => column !== name);
    void commit(next);
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
        {options.map((option, index) => {
          const position = selected.indexOf(option.name);
          const checked = position >= 0;
          const invalid = editor.schemaKnown && !eligibleNames.has(option.name);
          return (
            <div key={option.name} className="flex h-5 items-center gap-2">
              <Checkbox
                id={`${id}-${index}`}
                aria-label={t("detail.parameterEditor.selectColumn", { column: option.name })}
                checked={checked}
                className={disabled ? "cursor-wait" : undefined}
                aria-disabled={disabled}
                aria-invalid={invalid}
                onCheckedChange={(checked) => toggle(option.name, checked === true)}
              />
              <Label
                htmlFor={`${id}-${index}`}
                className={cn("min-w-0 flex-1", disabled && "cursor-wait")}
              >
                <span className={cn("truncate whitespace-pre", invalid && "text-destructive")}>
                  {option.name}
                </span>
                <span className="ml-2 text-xs text-muted-foreground">{option.dataType}</span>
                {invalid && (
                  <span className="text-xs text-destructive">
                    {t("detail.parameterEditor.unavailableColumn")}
                  </span>
                )}
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
      {editor.schemaKnown && editor.options.length === 0 && (
        <p className="text-xs text-muted-foreground">{t("detail.parameterEditor.noColumns")}</p>
      )}
      {(!editor.schemaKnown || name.length > 0) && (
        <div className="flex items-center gap-1">
          <Input
            aria-label={t("detail.parameterEditor.column")}
            placeholder={t("detail.parameterEditor.enterColumn")}
            value={name}
            aria-disabled={disabled}
            onChange={(event) => {
              if (!disabled) setName(event.target.value);
            }}
            onBlur={add}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing) return;
              if (event.key === "Enter") {
                event.preventDefault();
                add();
              }
              if (event.key === "Escape") {
                event.preventDefault();
                setName("");
              }
            }}
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={
              disabled ||
              name.length === 0 ||
              selected.includes(name) ||
              (editor.schemaKnown && !eligibleNames.has(name))
            }
            onClick={add}
          >
            {t("detail.parameterEditor.addColumn")}
          </Button>
        </div>
      )}
      <EditorMessages unavailable={editor.contextHint} errors={errors} />
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
    operator: editor.value?.operator ?? option?.operators[0] ?? "equal",
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
  const [projection, setProjection] = useState(editor.value);
  const [draft, setDraft] = useState(() => projectedPredicate(editor));
  if (projection !== editor.value) {
    setProjection(editor.value);
    setDraft(projectedPredicate(editor));
  }
  const { column, operator, literalType, value } = draft;
  const selectedColumn = editor.columns.find((option) => option.name === column);

  const operators = editor.schemaKnown ? (selectedColumn?.operators ?? []) : FILTER_OPERATORS;
  const literalTypes = editor.schemaKnown
    ? (selectedColumn?.literalTypes ?? [])
    : FILTER_LITERAL_TYPES;

  const reset = () => setDraft(projectedPredicate(latestEditor.current));
  const submit = (next: PredicateDraft) => {
    const option = editor.columns.find((candidate) => candidate.name === next.column);
    if (disabled || next.column.length === 0 || !next.operator) return;
    if (editor.schemaKnown && (!option || !option.operators.includes(next.operator))) return;
    const predicate: FilterPredicateDto = { column: next.column, operator: next.operator };
    if (operatorNeedsValue(next.operator)) {
      if (editor.schemaKnown && !option?.literalTypes.includes(next.literalType)) return;
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
        {editor.schemaKnown ? (
          <Select value={column} onValueChange={chooseColumn} disabled={disabled}>
            <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateColumn")}>
              <SelectValue placeholder={t("detail.parameterEditor.column")} />
            </SelectTrigger>
            <SelectContent data-parameter-editor={id}>
              {column && !selectedColumn && (
                <SelectItem value={column} disabled>
                  {column} — {t("detail.parameterEditor.unavailableColumn")}
                </SelectItem>
              )}
              {editor.columns.map((option) => (
                <SelectItem key={option.name} value={option.name}>
                  {option.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : (
          <Input
            aria-label={t("detail.parameterEditor.predicateColumn")}
            value={column}
            disabled={disabled}
            onChange={(event) => setDraft({ ...draft, column: event.target.value })}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing) return;
              if (event.key === "Enter") {
                event.preventDefault();
                submit(draft);
              }
              if (event.key === "Escape") {
                event.preventDefault();
                reset();
              }
            }}
          />
        )}
        {editor.schemaKnown && editor.columns.length === 0 && (
          <p className="text-xs text-muted-foreground">{t("detail.parameterEditor.noColumns")}</p>
        )}
      </div>
      <div className="space-y-1.5">
        <Label>{t("detail.parameterEditor.operator")}</Label>
        <Select
          key={column}
          value={operator ?? ""}
          onValueChange={(value) => change({ ...draft, operator: value as FilterOperatorDto })}
          disabled={disabled || operators.length === 0}
        >
          <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateOperator")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent data-parameter-editor={id}>
            {operator && !operators.includes(operator) && (
              <SelectItem value={operator} disabled>
                {operator}
              </SelectItem>
            )}
            {operators.map((option) => (
              <SelectItem key={option} value={option}>
                {option}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {operatorNeedsValue(operator) &&
        (literalTypes.length > 1 || !literalTypes.includes(literalType)) && (
          <div className="space-y-1.5">
            <Label>{t("detail.parameterEditor.valueType")}</Label>
            <Select
              value={literalType}
              onValueChange={(type) => {
                const nextType = type as FilterLiteralDto["type"];
                change({
                  ...draft,
                  literalType: nextType,
                  value: literalValue(undefined, nextType),
                });
              }}
              disabled={disabled}
            >
              <SelectTrigger size="sm" aria-label={t("detail.parameterEditor.predicateValueType")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent data-parameter-editor={id}>
                {!literalTypes.includes(literalType) && (
                  <SelectItem value={literalType} disabled>
                    {literalType}
                  </SelectItem>
                )}
                {literalTypes.map((type) => (
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
      <EditorMessages unavailable={editor.contextHint} errors={errors} />
    </div>
  );
}
