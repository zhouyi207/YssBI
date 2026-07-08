import { useTranslation } from 'react-i18next';
import type { FunctionPinSpec, FunctionSignaturePatch } from '@/shared/types';
import type { FunctionCallSiteDTO } from '@/shared/types/dto';
import type { VariableListEntry } from '@/features/core/variable/variableScopeSelectors';
import { openGraphResource, resolveGraphResourceMeta } from '@/features/application/editor/openGraphResource';
import { focusDetailOnNode } from '@/features/core/editor/detail/detailFocusCommands';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { PinEditor } from '../shared/PinEditor';
import { DetailForm, DetailNameField, DetailReadonlyField } from '../shared/DetailForm';
import { GraphLocalVariablesSection } from '../shared/GraphLocalVariablesSection';
import { detailMetaTextClass, detailSectionTitleClass, detailSubsectionTitleClass } from '../shared/detailStyles';

interface FunctionDetailPanelProps {
  fn: {
    path: string;
    name: string;
    inputs?: FunctionPinSpec[];
    outputs?: FunctionPinSpec[];
  };
  callSites?: FunctionCallSiteDTO[];
  callSitesLoading?: boolean;
  localVariables?: VariableListEntry[];
  onSelectLocalVariable?: (id: string) => void;
  onAddLocalVariable?: () => void;
  onRename: (name: string) => void;
  onSignatureChange: (patch: FunctionSignaturePatch) => void;
}

export function FunctionDetailPanel({
  fn,
  callSites = [],
  callSitesLoading = false,
  localVariables = [],
  onSelectLocalVariable,
  onAddLocalVariable,
  onRename,
  onSignatureChange,
}: FunctionDetailPanelProps) {
  const { t } = useTranslation();

  const totalCalls = callSites.reduce((sum, site) => sum + site.nodeIds.length, 0);

  const handleOpenCallSite = (callerGraphPath: string, nodeId: string) => {
    void (async () => {
      await openGraphResource(callerGraphPath);
      focusDetailOnNode(nodeId);
    })();
  };

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: fn.name })}>
      <DetailForm>
        <DetailNameField
          label={t('detail.fields.name')}
          value={fn.name}
          onCommit={onRename}
        />
        <DetailReadonlyField label={t('detail.fields.type')} className="italic">
          {t('detail.typeLabels.function')}
        </DetailReadonlyField>
      </DetailForm>
      <GraphLocalVariablesSection
        variables={localVariables}
        onSelectVariable={(id) => onSelectLocalVariable?.(id)}
        onAddVariable={onAddLocalVariable}
      />
      <section className="mt-4 space-y-2">
        <h3 className={detailSectionTitleClass}>
          {t('detail.callSites.title', { count: totalCalls })}
        </h3>
        {callSitesLoading ? (
          <p className={detailMetaTextClass}>{t('detail.callSites.loading')}</p>
        ) : callSites.length === 0 ? (
          <p className={detailMetaTextClass}>{t('detail.callSites.empty')}</p>
        ) : (
          <ul className="space-y-1">
            {callSites.map((site) => {
              const meta = resolveGraphResourceMeta(site.callerGraphPath);
              const name = meta?.name ?? site.callerGraphPath;
              return (
                <li key={site.callerGraphPath}>
                  <div className={detailSubsectionTitleClass}>{name}</div>
                  <ul className="mt-1 space-y-0.5">
                    {site.nodeIds.map((nodeId) => (
                      <li key={nodeId}>
                        <button
                          type="button"
                          className="text-left text-xs text-[var(--accent-color)] hover:underline"
                          onClick={() => handleOpenCallSite(site.callerGraphPath, nodeId)}
                        >
                          {t('detail.callSites.openCaller', { nodeId })}
                        </button>
                      </li>
                    ))}
                  </ul>
                </li>
              );
            })}
          </ul>
        )}
      </section>
      <PinEditor
        title={t('detail.pinEditor.inputs')}
        emptyMessage={t('detail.pinEditor.noInputs')}
        pins={fn.inputs ?? []}
        onChange={(inputs) => onSignatureChange({ inputs })}
      />
      <PinEditor
        title={t('detail.pinEditor.outputs')}
        emptyMessage={t('detail.pinEditor.noOutputs')}
        pins={fn.outputs ?? []}
        onChange={(outputs) => onSignatureChange({ outputs })}
      />
    </DetailPanelShell>
  );
}
