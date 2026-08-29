import { FormEvent, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type {
  FilterLiteralDto,
  FilterOperatorDto,
  FilterPredicateDto,
  SchemaAwareParameterEditorDto,
} from '@/shared/types/domain/editorProjection';

interface EditorProps<TEditor, TValue> {
  editor: TEditor;
  errors: readonly string[];
  disabled?: boolean;
  onCommit(value: TValue): void | Promise<void>;
}

type ProjectEditor = Extract<SchemaAwareParameterEditorDto, { kind: 'projectColumns' }>;
type FilterEditor = Extract<SchemaAwareParameterEditorDto, { kind: 'filterPredicate' }>;

function EditorMessages({ unavailable, errors }: {
  unavailable?: string | null;
  errors: readonly string[];
}) {
  return (
    <div className="space-y-1 text-xs" aria-live="polite">
      {unavailable && <p className="text-muted-foreground">{unavailable}</p>}
      {errors.map((error) => <p key={error} className="text-destructive">{error}</p>)}
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
  const [selected, setSelected] = useState<string[]>(editor.value);
  useEffect(() => setSelected(editor.value), [editor.value]);

  if (!editor.available) {
    return <EditorMessages unavailable={editor.unavailableReason} errors={errors} />;
  }

  const toggle = (name: string, checked: boolean) => {
    setSelected((current) => checked
      ? current.includes(name) ? current : [...current, name]
      : current.filter((column) => column !== name));
  };
  const move = (name: string, offset: -1 | 1) => {
    setSelected((current) => {
      const from = current.indexOf(name);
      const to = from + offset;
      if (from < 0 || to < 0 || to >= current.length) return current;
      const reordered = [...current];
      [reordered[from], reordered[to]] = [reordered[to], reordered[from]];
      return reordered;
    });
  };

  return (
    <form className="space-y-3" onSubmit={(event) => {
      event.preventDefault();
      if (selected.length > 0) void onCommit(selected);
    }}>
      <div className="space-y-2">
        {editor.options.map((option) => (
          <div key={option.name} className="flex items-center gap-2">
            <Checkbox
              id={`project-column-${option.name}`}
              aria-label={t('detail.parameterEditor.selectColumn', { column: option.name })}
              checked={selected.includes(option.name)}
              disabled={disabled}
              onCheckedChange={(checked) => toggle(option.name, checked === true)}
            />
            <Label htmlFor={`project-column-${option.name}`} className="min-w-0 flex-1">
              <span className="truncate">{option.name}</span>
              <span className="ml-2 text-xs text-muted-foreground">{option.dataType}</span>
            </Label>
            {selected.includes(option.name) && (
              <div className="flex items-center gap-0.5">
                <span className="mr-1 text-xs tabular-nums text-muted-foreground">
                  {selected.indexOf(option.name) + 1}
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t('detail.parameterEditor.moveColumnUp', { column: option.name })}
                  disabled={disabled || selected.indexOf(option.name) === 0}
                  onClick={() => move(option.name, -1)}
                >
                  ↑
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t('detail.parameterEditor.moveColumnDown', { column: option.name })}
                  disabled={disabled || selected.indexOf(option.name) === selected.length - 1}
                  onClick={() => move(option.name, 1)}
                >
                  ↓
                </Button>
              </div>
            )}
          </div>
        ))}
      </div>
      <EditorMessages errors={errors} />
      <Button type="submit" size="sm" disabled={disabled || selected.length === 0}>
        {t('detail.parameterEditor.apply')}
      </Button>
    </form>
  );
}

function defaultLiteralType(
  column: FilterEditor['columns'][number] | undefined,
): FilterLiteralDto['type'] {
  return column?.literalTypes[0] ?? 'string';
}

function issuedOperator(
  column: FilterEditor['columns'][number] | undefined,
  requested?: FilterOperatorDto,
): FilterOperatorDto | undefined {
  return requested && column?.operators.includes(requested)
    ? requested
    : column?.operators[0];
}

function operatorNeedsValue(operator: FilterOperatorDto | undefined): boolean {
  return operator !== undefined && operator !== 'isNull' && operator !== 'isNotNull';
}

function literalValue(
  literal: FilterLiteralDto | undefined,
  type: FilterLiteralDto['type'],
): string {
  if (!literal) return type === 'boolean' ? 'false' : '';
  return literal.type === 'boolean' ? String(literal.value) : literal.value;
}

export function FilterPredicateEditor({
  editor,
  errors,
  disabled = false,
  onCommit,
}: EditorProps<FilterEditor, FilterPredicateDto>) {
  const { t } = useTranslation();
  const initialColumn = editor.value?.column ?? editor.columns[0]?.name ?? '';
  const [column, setColumn] = useState(initialColumn);
  const selectedColumn = useMemo(
    () => editor.columns.find((option) => option.name === column) ?? editor.columns[0],
    [column, editor.columns],
  );
  const initialOperator = issuedOperator(selectedColumn, editor.value?.operator);
  const [operator, setOperator] = useState<FilterOperatorDto | undefined>(initialOperator);
  const initialType = editor.value?.value?.type
    ?? defaultLiteralType(selectedColumn);
  const [literalType, setLiteralType] = useState<FilterLiteralDto['type']>(initialType);
  const [value, setValue] = useState(literalValue(editor.value?.value, initialType));

  useEffect(() => {
    const nextColumn = editor.value?.column ?? editor.columns[0]?.name ?? '';
    const option = editor.columns.find((candidate) => candidate.name === nextColumn);
    const nextType = editor.value?.value?.type ?? defaultLiteralType(option);
    setColumn(nextColumn);
    setOperator(issuedOperator(option, editor.value?.operator));
    setLiteralType(nextType);
    setValue(literalValue(editor.value?.value, nextType));
  }, [editor]);

  if (!editor.available) {
    return <EditorMessages unavailable={editor.unavailableReason} errors={errors} />;
  }

  const chooseColumn = (name: string) => {
    const option = editor.columns.find((candidate) => candidate.name === name);
    const nextType = defaultLiteralType(option);
    setColumn(name);
    setOperator(issuedOperator(option));
    setLiteralType(nextType);
    setValue(literalValue(undefined, nextType));
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!selectedColumn || !operator || !selectedColumn.operators.includes(operator)) return;
    if (!operatorNeedsValue(operator)) {
      void onCommit({ column: selectedColumn.name, operator });
      return;
    }
    if (value.length === 0) return;
    const literal: FilterLiteralDto = literalType === 'boolean'
      ? { type: 'boolean', value: value === 'true' }
      : { type: literalType, value };
    void onCommit({ column: selectedColumn.name, operator, value: literal });
  };

  return (
    <form className="space-y-3" onSubmit={submit}>
      <div className="space-y-1.5">
        <Label>{t('detail.parameterEditor.column')}</Label>
        <Select value={column} onValueChange={chooseColumn} disabled={disabled}>
          <SelectTrigger size="sm" aria-label={t('detail.parameterEditor.predicateColumn')}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {editor.columns.map((option) => (
              <SelectItem key={option.name} value={option.name}>{option.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-1.5">
        <Label>{t('detail.parameterEditor.operator')}</Label>
        <Select
          key={column}
          value={operator ?? ''}
          onValueChange={(value) => setOperator(value as FilterOperatorDto)}
          disabled={disabled || !selectedColumn || selectedColumn.operators.length === 0}
        >
          <SelectTrigger size="sm" aria-label={t('detail.parameterEditor.predicateOperator')}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {selectedColumn?.operators.map((option) => (
              <SelectItem key={option} value={option}>{option}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {operatorNeedsValue(operator) && (selectedColumn?.literalTypes.length ?? 0) > 1 && (
        <div className="space-y-1.5">
          <Label>{t('detail.parameterEditor.valueType')}</Label>
          <Select
            value={literalType}
            onValueChange={(type) => {
              const nextType = type as FilterLiteralDto['type'];
              setLiteralType(nextType);
              setValue(literalValue(undefined, nextType));
            }}
            disabled={disabled}
          >
            <SelectTrigger size="sm" aria-label={t('detail.parameterEditor.predicateValueType')}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {selectedColumn?.literalTypes.map((type) => (
                <SelectItem key={type} value={type}>{type}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      {operatorNeedsValue(operator) && literalType === 'boolean' && (
        <Select value={value} onValueChange={setValue} disabled={disabled}>
          <SelectTrigger size="sm" aria-label={t('detail.parameterEditor.predicateValue')}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="true">true</SelectItem>
            <SelectItem value="false">false</SelectItem>
          </SelectContent>
        </Select>
      )}
      {operatorNeedsValue(operator) && literalType !== 'boolean' && (
        <Input
          aria-label={t('detail.parameterEditor.predicateValue')}
          value={value}
          disabled={disabled}
          onChange={(event) => setValue(event.target.value)}
        />
      )}
      <EditorMessages errors={errors} />
      <Button
        type="submit"
        size="sm"
        disabled={disabled
          || !selectedColumn
          || !operator
          || !selectedColumn.operators.includes(operator)
          || (operatorNeedsValue(operator) && value.length === 0)}
      >
        {t('detail.parameterEditor.apply')}
      </Button>
    </form>
  );
}
