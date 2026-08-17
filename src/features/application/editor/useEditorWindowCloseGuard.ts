import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { i18n } from '@/app/i18n';
import { collectDirtyGraphTabs } from '@/features/core/layout/tabDirty';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';

/** Keeps dirty-document close protection attached for the editor window's lifetime. */
export function useEditorWindowCloseGuard(): void {
  useEffect(() => {
    const appWindow = getCurrentWindow();
    let cancelled = false;
    let unlistenClose: (() => void) | null = null;
    let allowDestructiveClose = false;

    const setupCloseListener = async () => {
      try {
        const unlisten = await appWindow.onCloseRequested(async (event) => {
          if (allowDestructiveClose) return;

          const dirty = collectDirtyGraphTabs();
          if (dirty.length === 0) return;

          event.preventDefault();

          const titles = dirty.map((tab) => `• ${tab.title}`).join('\n');
          const choice = await uiStore.confirm3({
            title: i18n.t('editor.unsavedTitle', { defaultValue: '保存更改？' }),
            message: i18n.t('editor.unsavedMessage', {
              defaultValue: `以下 {{count}} 个图存在未保存修改：\n{{titles}}\n\n关闭前是否保存？`,
              count: dirty.length,
              titles,
            }),
            confirmText: i18n.t('editor.unsavedSaveAll', { defaultValue: '全部保存' }),
            discardText: i18n.t('editor.unsavedDiscard', { defaultValue: '不保存' }),
            cancelText: i18n.t('common.cancel', { defaultValue: '取消' }),
            type: 'info',
          });

          if (choice === 'cancel') return;

          if (choice === 'confirm') {
            const saved = await saveAllDirtyGraphs();
            if (!saved) return;
          }

          allowDestructiveClose = true;
          try {
            await appWindow.close();
          } catch (error) {
            logger.app.error(
              `Failed to close window after dirty-tab decision: ${error instanceof Error ? error.message : String(error)}`,
              'EditorWindow',
            );
            allowDestructiveClose = false;
          }
        });

        if (cancelled) {
          unlisten();
        } else {
          unlistenClose = unlisten;
        }
      } catch (error) {
        logger.app.warn(
          `Failed to attach editor window close guard: ${error instanceof Error ? error.message : String(error)}`,
          'EditorWindow',
        );
      }
    };

    void setupCloseListener();

    return () => {
      cancelled = true;
      unlistenClose?.();
      unlistenClose = null;
    };
  }, []);
}
