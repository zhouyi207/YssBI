import { useTranslation } from 'react-i18next';
import type { VariableListEntry } from '@/features/core/variable/variableScopeSelectors';
import { detailMetaTextClass, detailSectionTitleClass } from './detailStyles';

interface GraphLocalVariablesSectionProps {
  variables: VariableListEntry[];
  onSelectVariable: (id: string) => void;
  onAddVariable?: () => void;
}

export function GraphLocalVariablesSection({
  variables,
  onSelectVariable,
  onAddVariable,
}: GraphLocalVariablesSectionProps) {
  const { t } = useTranslation();

  return (
    <section className="mt-4 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <h3 className={detailSectionTitleClass}>
          {t('detail.localVariables.title', { count: variables.length })}
        </h3>
        {onAddVariable ? (
          <button
            type="button"
            className="text-xs text-[var(--accent-color)] hover:underline"
            onClick={onAddVariable}
          >
            {t('detail.localVariables.add')}
          </button>
        ) : null}
      </div>
      {variables.length === 0 ? (
        <p className={detailMetaTextClass}>{t('detail.localVariables.empty')}</p>
      ) : (
        <ul className="space-y-1">
          {variables.map((variable) => (
            <li key={variable.id}>
              <button
                type="button"
                className="flex w-full items-center justify-between gap-2 text-left text-xs hover:text-[var(--accent-color)]"
                onClick={() => onSelectVariable(variable.id)}
              >
                <span className="truncate">{variable.name}</span>
                <span className={detailMetaTextClass}>{variable.typeLabel}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
