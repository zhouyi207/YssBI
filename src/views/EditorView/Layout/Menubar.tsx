import { Fragment, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import {
  useEditorHistoryAvailability,
  useEditorSessionCommandsContext,
} from '@/features/application/editor';
import { EDITOR_MUTATION_CAPABILITIES } from '@/features/application/editor/editorMutationAvailability';
import { useMenubar } from '@/features/application/menubar';
import {
  buildViewMenuItems,
  type MenubarMenuItem,
} from '@/features/application/menubar/menubarViewItems';
import { getRememberedColorTheme } from '@/features/application/settings/colorThemePresets';
import {
  openExternalUrlWithDialog,
  useCurrentWindowActions,
  useCustomTitleBar,
} from '@/features/application/window';
import { useActiveProjectPath } from '@/features/application/project/projectSession';
import { useSettingsRead } from '@/features/core/settings/read';
import { settingsUi } from '@/features/core/settings/ui';
import { APP_LINKS } from '@/shared/config-default';
import {
  Menubar as ShadcnMenubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarGroup,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarShortcut,
  MenubarTrigger,
} from '@/components/ui/menubar';
import { BrandLockup } from '@/shared/ui/BrandMark';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowMenuBar } from '@/shared/ui/WindowChrome';
import { AboutModal } from './AboutModal';

export type MenuItem = MenubarMenuItem;

export function buildEditMenuItems(
  translate: (key: string) => string,
  state: {
    activeTabId: string | null;
    canUndo: boolean;
    canRedo: boolean;
    editorCommandAuthorized: boolean;
  },
  actions: {
    undo: () => void;
    redo: () => void;
    cut: () => void;
    copy: () => void;
    paste: () => void;
    deleteSelected: () => void;
  },
): MenuItem[] {
  const authorized = state.editorCommandAuthorized;
  return [
    {
      label: translate('common.undo'),
      shortcut: 'Ctrl+Z',
      onClick: authorized && state.canUndo ? actions.undo : undefined,
    },
    {
      label: translate('common.redo'),
      shortcut: 'Ctrl+Y',
      onClick: authorized && state.canRedo ? actions.redo : undefined,
    },
    { label: '-', type: 'separator' },
    {
      label: translate('menubar.cut'),
      shortcut: 'Ctrl+X',
      onClick: authorized ? actions.cut : undefined,
    },
    {
      label: translate('menubar.copy'),
      shortcut: 'Ctrl+C',
      onClick: authorized ? actions.copy : undefined,
    },
    {
      label: translate('menubar.paste'),
      shortcut: 'Ctrl+V',
      onClick: authorized && EDITOR_MUTATION_CAPABILITIES.pasteNodes
        ? actions.paste
        : undefined,
    },
    { label: '-', type: 'separator' },
    {
      label: translate('common.delete'),
      shortcut: 'Del',
      onClick: authorized ? actions.deleteSelected : undefined,
    },
  ];
}

export function buildFileMenuItems(
  translate: (key: string) => string,
  state: {
    projectAvailable: boolean;
    editorCommandAuthorized: boolean;
  },
  actions: {
    addEvent: () => void;
    addFunction: () => void;
    openProject: () => void;
    closeProject: () => void;
    saveGraph: () => void;
    saveGraphAs: () => void;
  },
): MenuItem[] {
  return [
    {
      label: translate('menubar.newEventGraph'),
      shortcut: 'Ctrl+N',
      onClick: actions.addEvent,
    },
    { label: translate('menubar.newFunction'), onClick: actions.addFunction },
    { label: '-', type: 'separator' },
    {
      label: translate('menubar.openProject'),
      shortcut: 'Ctrl+O',
      onClick: actions.openProject,
    },
    { label: translate('menubar.closeProject'), onClick: actions.closeProject },
    { label: '-', type: 'separator' },
    {
      label: translate('menubar.saveProject'),
      shortcut: 'Ctrl+S',
      onClick: state.projectAvailable && state.editorCommandAuthorized
        ? actions.saveGraph
        : undefined,
    },
    {
      label: translate('menubar.saveProjectAs'),
      shortcut: 'Ctrl+Shift+S',
      onClick: state.projectAvailable ? actions.saveGraphAs : undefined,
    },
  ];
}

export function buildWindowMenuItems(
  translate: (key: string) => string,
  editorCommandAuthorized: boolean,
  actions: {
    splitRight: () => void;
    splitDown: () => void;
    openLogsWindow: () => void;
  },
): MenuItem[] {
  return [
    {
      label: translate('menubar.splitEditorRight'),
      onClick: editorCommandAuthorized ? actions.splitRight : undefined,
    },
    {
      label: translate('menubar.splitEditorDown'),
      onClick: editorCommandAuthorized ? actions.splitDown : undefined,
    },
    { label: '-', type: 'separator' },
    {
      label: translate('menubar.openLogsInNewWindow'),
      onClick: actions.openLogsWindow,
    },
  ];
}

interface MenuButtonProps {
  id: string;
  label: string;
  items: MenuItem[];
}

function selectMenuItem(
  event: Event,
  onClick: (() => void) | undefined,
): void {
  if (!onClick) {
    event.preventDefault();
    return;
  }
  onClick();
}

const MenuButton = ({ id, label, items }: MenuButtonProps) => {
  const sections = items.reduce<MenuItem[][]>((groups, item) => {
    if (item.type === 'separator' || item.label === '-') {
      if (groups[groups.length - 1]?.length) groups.push([]);
      return groups;
    }

    groups[groups.length - 1]?.push(item);
    return groups;
  }, [[]]);

  return (
    <MenubarMenu value={id}>
      <MenubarTrigger>{label}</MenubarTrigger>
      <MenubarContent>
        {sections.map((section, sectionIndex) => (
          <Fragment key={`${id}-section-${sectionIndex}`}>
            {sectionIndex > 0 ? <MenubarSeparator /> : null}
            <MenubarGroup>
              {section.map((item, itemIndex) => {
                const content = (
                  <>
                    <span className="flex-1">{item.label}</span>
                    {item.shortcut
                      ? <MenubarShortcut>{item.shortcut}</MenubarShortcut>
                      : null}
                  </>
                );

                if (item.type === 'checkbox') {
                  return (
                    <MenubarCheckboxItem
                      key={`${id}-${sectionIndex}-${itemIndex}`}
                      checked={item.checked}
                      disabled={!item.onClick}
                      onSelect={(event) => selectMenuItem(event, item.onClick)}
                    >
                      {content}
                    </MenubarCheckboxItem>
                  );
                }

                return (
                  <MenubarItem
                    key={`${id}-${sectionIndex}-${itemIndex}`}
                    disabled={!item.onClick}
                    onSelect={(event) => selectMenuItem(event, item.onClick)}
                  >
                    {content}
                  </MenubarItem>
                );
              })}
            </MenubarGroup>
          </Fragment>
        ))}
      </MenubarContent>
    </MenubarMenu>
  );
};

export function EditorMenuBar({ menus }: { menus: readonly MenuButtonProps[] }) {
  return (
    <ShadcnMenubar className="border-0">
      {menus.map((menu) => <MenuButton key={menu.id} {...menu} />)}
    </ShadcnMenubar>
  );
}

export function Menubar() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [aboutOpen, setAboutOpen] = useState(false);
  const {
    importGraph,
    saveGraph,
    saveGraphAs,
    undo,
    redo,
    copy,
    paste,
    cut,
    deleteSelected,
    addEvent,
    addFunction,
    addWorksheet,
  } = useEditorSessionCommandsContext();
  const { canUndo, canRedo, activeTabId } = useEditorHistoryAvailability();
  const {
    openSettings,
    editorCommandAuthorized,
    viewState,
    viewActions,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDatabaseEditor,
    handleOpenLogs,
  } = useMenubar();

  const currentPath = useActiveProjectPath();
  const projectAvailable = Boolean(currentPath);
  const themeMode = useSettingsRead((state) => state.theme.mode ?? 'dark');
  const appearance = useSettingsRead((state) => state.appearance);
  const updateAppearance = settingsUi.updateAppearance;
  const isLightTheme = themeMode === 'light';
  const windowActions = useCurrentWindowActions();
  const customChrome = useCustomTitleBar();

  const toggleThemeMode = () => {
    const nextMode = isLightTheme ? 'dark' : 'light';
    updateAppearance({
      colorTheme: getRememberedColorTheme(
        nextMode,
        appearance.lastLightColorTheme,
        appearance.lastDarkColorTheme,
      ),
    });
  };

  const fileItems = buildFileMenuItems(
    t,
    { projectAvailable, editorCommandAuthorized },
    {
      addEvent: () => void addEvent(undefined, { openAfterCreate: true }),
      addFunction: () => void addFunction(undefined, { openAfterCreate: true }),
      openProject: () => void importGraph(),
      closeProject: () => navigate('/projects'),
      saveGraph: () => void saveGraph(),
      saveGraphAs: () => void saveGraphAs(),
    },
  );

  const editItems = buildEditMenuItems(
    t,
    { activeTabId, canUndo, canRedo, editorCommandAuthorized },
    {
      undo: () => void undo(),
      redo: () => void redo(),
      cut: () => void cut(),
      copy: () => void copy(),
      paste: () => void paste(),
      deleteSelected: () => void deleteSelected(),
    },
  );

  const dataItems: MenuItem[] = [
    { label: t('menubar.manageVariables') },
    { label: t('menubar.importData'), onClick: handleImportData },
    { label: t('menubar.databaseEditor'), onClick: handleDatabaseEditor },
    { label: t('menubar.newWorksheet'), onClick: () => void addWorksheet() },
    { label: '-', type: 'separator' },
    { label: t('menubar.schemaViewer') },
  ];

  const viewItems = buildViewMenuItems(t, viewState, viewActions);
  const windowItems = buildWindowMenuItems(t, editorCommandAuthorized, {
    splitRight: handleSplitRight,
    splitDown: handleSplitDown,
    openLogsWindow: handleOpenLogs,
  });

  const toolItems: MenuItem[] = [
    { label: t('menubar.debugger') },
    { label: t('menubar.profiler') },
    { label: '-', type: 'separator' },
    { label: t('menubar.settings'), shortcut: 'Ctrl+,', onClick: openSettings },
  ];

  const helpItems: MenuItem[] = [
    {
      label: t('menubar.documentation'),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.documentation, t),
    },
    { label: '-', type: 'separator' },
    {
      label: t('menubar.releaseNotes'),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.releaseNotes, t),
    },
    {
      label: t('menubar.githubRepository'),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.repository, t),
    },
    {
      label: t('menubar.reportIssue'),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.reportIssue, t),
    },
    { label: '-', type: 'separator' },
    { label: t('menubar.about'), onClick: () => setAboutOpen(true) },
  ];
  const menus: MenuButtonProps[] = [
    { id: 'file', label: t('menubar.file'), items: fileItems },
    { id: 'edit', label: t('menubar.edit'), items: editItems },
    { id: 'data', label: t('menubar.data'), items: dataItems },
    { id: 'view', label: t('menubar.view'), items: viewItems },
    { id: 'window', label: t('menubar.window'), items: windowItems },
    { id: 'tools', label: t('menubar.tools'), items: toolItems },
    { id: 'help', label: t('menubar.help'), items: helpItems },
  ];

  return (
    <>
      <WindowMenuBar
        customChrome={customChrome}
        toolbar={
          <>
            <ToolbarIconButton
              variant="ghost"
              size="icon-lg"
              onClick={toggleThemeMode}
              className="self-center text-muted-foreground"
              tooltip={isLightTheme ? t('menubar.switchToDark') : t('menubar.switchToLight')}
              aria-label={isLightTheme ? t('menubar.switchToDark') : t('menubar.switchToLight')}
            >
              {isLightTheme ? (
                <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12.8A8.5 8.5 0 1111.2 3a7 7 0 009.8 9.8z" />
                </svg>
              ) : (
                <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 3v2m0 14v2m9-9h-2M5 12H3m15.36-6.36-1.42 1.42M7.06 16.94l-1.42 1.42m12.72 0-1.42-1.42M7.06 7.06 5.64 5.64" />
                  <circle cx="12" cy="12" r="4" strokeWidth={2} />
                </svg>
              )}
            </ToolbarIconButton>
          </>
        }
        windowActions={(
          <WindowChromeControls
            maximized={windowActions.maximized}
            minimize={windowActions.minimize}
            toggleMaximize={windowActions.toggleMaximize}
            close={windowActions.close}
          />
        )}
      >
        <BrandLockup className="pointer-events-none self-center px-4" />

        <EditorMenuBar menus={menus} />
      </WindowMenuBar>
      <AboutModal open={aboutOpen} onOpenChange={setAboutOpen} />
    </>
  );
}
