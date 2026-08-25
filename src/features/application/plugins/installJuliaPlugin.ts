import type { TFunction } from 'i18next';

import { summarizeUserError } from '@/features/application/userErrorSummary';
import { usePluginStore } from '@/features/core/plugins/pluginStore';
import { uiStore } from '@/features/core/ui/UIStore';
import {
  JuliaRuntimeService,
} from '@/services/julia/juliaRuntimeService';
import { JULIA_PLUGIN_ID } from './pluginCatalog';

export async function installJuliaPlugin(t: TFunction): Promise<boolean> {
  const confirmed = await uiStore.confirm({
    title: t('julia.install.title'),
    message: t('julia.install.message'),
    confirmText: t('julia.install.confirm'),
  });
  if (!confirmed) {
    return false;
  }

  uiStore.startProgress({
    stage: t('julia.install.preparing'),
    detail: t('julia.install.preparingDetail'),
  });

  let failure: { message: string; incidentId: string | null } | null = null;
  try {
    const nextStatus = await JuliaRuntimeService.install();
    if (nextStatus.state !== 'ready') {
      failure = {
        message: t(
          nextStatus.state === 'invalid'
            ? 'julia.status.invalid'
            : 'julia.status.notInstalled',
        ),
        incidentId: null,
      };
    }
  } catch (error) {
    failure = summarizeUserError(error, t);
  } finally {
    uiStore.finishProgress();
  }

  if (failure) {
    await uiStore.alert({
      title: t('julia.install.failed'),
      message: t('notifications.julia.installFailed', { error: failure.message }),
      closeText: t('common.close'),
      type: 'error',
      incidentId: failure.incidentId,
      incidentLabel: t('common.incidentId'),
    });
    return false;
  }

  usePluginStore.getState().installPlugin(JULIA_PLUGIN_ID);
  return true;
}
