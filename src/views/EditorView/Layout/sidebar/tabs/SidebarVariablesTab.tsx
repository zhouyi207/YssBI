import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { DEFAULT_VARIABLE_NAME } from '@/shared/constants/defaultResourceNames';
import { partitionVariableCatalog } from '@/features/core/variable/variableScopeSelectors';
import type { Variable } from '@/shared/types/domain/variable';
import {
  buildVariablesSidebarModel,
  useSidebarSectionExpandSnapshot,
  useSidebarStore,
} from '@/features/core/sidebar';
import { useSidebarVariableScope } from '../useSidebarVariableScope';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';
import { SidebarFlatRowPanel } from '../sections/SidebarFlatRowPanel';
import { noopSidebarHandler } from '../sections/sidebarFlatRowContext';

export function SidebarVariablesTab({
  variables,
  onAddVariable,
  onSectionContextMenu,
  onVariableContextMenu,
}: {
  variables: Record<string, Variable>;
  onAddVariable: (name: string, dataType: string, isGlobal: boolean) => void;
  onSectionContextMenu: (e: React.MouseEvent, isGlobal: boolean) => void;
  onVariableContextMenu: (e: React.MouseEvent, id: string, name: string) => void;
}) {
  const { t } = useTranslation();
  const { scopePath, graphType } = useSidebarVariableScope();
  const sectionExpanded = useSidebarSectionExpandSnapshot('variablesLocal', 'variablesGlobal');
  const toggleSection = useSidebarStore((s) => s.toggleSection);

  const { global: variablesGlobal, local: localVariables } = useMemo(
    () =>
      partitionVariableCatalog(
        variables,
        graphType && scopePath ? { graphPath: scopePath, graphKind: graphType } : undefined,
      ),
    [variables, scopePath, graphType],
  );

  const model = useMemo(
    () =>
      buildVariablesSidebarModel({
        localVariables,
        globalVariables: variablesGlobal,
        hasActiveGraph: Boolean(graphType),
        expandedSections: sectionExpanded,
        labels: {
          local: t('sidebar.sections.local'),
          global: t('sidebar.sections.global'),
          noLocal: t('sidebar.noLocalVariables'),
          noGlobal: t('sidebar.noGlobalVariables'),
          noActiveGraph: t('sidebar.noActiveGraph'),
        },
      }),
    [graphType, localVariables, sectionExpanded, t, variablesGlobal],
  );

  const sectionActions = useMemo(
    () => ({
      variablesLocal: {
        onAdd: graphType ? () => onAddVariable(DEFAULT_VARIABLE_NAME, 'Int64', false) : undefined,
        addAriaLabel: t('contextMenu.sidebar.newVariable'),
        onHeaderContextMenu: (e: React.MouseEvent) => onSectionContextMenu(e, false),
        onContentContextMenu: (e: React.MouseEvent) => onSectionContextMenu(e, false),
      },
      variablesGlobal: {
        onAdd: () => onAddVariable(DEFAULT_VARIABLE_NAME, 'Int64', true),
        addAriaLabel: t('contextMenu.sidebar.newVariable'),
        onHeaderContextMenu: (e: React.MouseEvent) => onSectionContextMenu(e, true),
        onContentContextMenu: (e: React.MouseEvent) => onSectionContextMenu(e, true),
      },
    }),
    [graphType, onAddVariable, onSectionContextMenu, t],
  );

  return (
    <SidebarTabPanel>
      <SidebarFlatRowPanel
        model={model}
        sectionActions={sectionActions}
        onToggleSection={toggleSection}
        onToggleGroup={noopSidebarHandler}
        onVariableContextMenu={onVariableContextMenu}
      />
    </SidebarTabPanel>
  );
}
