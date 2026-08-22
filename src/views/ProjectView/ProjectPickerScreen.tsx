import { useCallback, useEffect, useMemo, useState, type MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { useProjectPicker, type ManagedProject } from '@/features/application/project';
import { usePersistedWindow } from '@/features/application/window';
import { ActionMenu, usePositionedActionMenu } from '@/shared/ui/actionMenu';
import { DeleteProjectConfirmDialog } from './DeleteProjectConfirmDialog';
import { NewProjectModal } from './NewProjectModal';
import { ProjectLibrary } from './ProjectLibrary';
import { ProjectPickerActionPanel } from './ProjectPickerActionPanel';
import { ProjectPickerHero } from './ProjectPickerHero';
import { ProjectPickerPageIssueAlert } from './ProjectPickerPageIssueAlert';
import { ProjectPickerTitleBar, ProjectSettingsDialog } from './ProjectPickerChrome';
import { buildProjectPickerContextMenuSections } from './projectPickerContextMenu';
import type { ProjectPickerContextMenuTarget } from './projectPickerContextMenu';
import {
  sortAndFilterProjects,
  type ProjectSortMode,
} from './projectPickerViewUtils';

export function ProjectPickerScreen() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  usePersistedWindow('main');
  const {
    busy,
    currentProjectId,
    projects,
    pageIssue,
    dismissPageIssue,
    createProject,
    importProjectFromDisk,
    openRecentProject,
    refresh,
    scanProjectsFromFolder,
    cleanupInvalidProjects,
    removeProject,
    deleteProjectFiles,
    toggleFavorite,
    revealProjectInExplorer,
  } = useProjectPicker();
  const [selectedId, setSelectedId] = useState<string | null>(currentProjectId);
  const [filterQuery, setFilterQuery] = useState('');
  const [sortMode, setSortMode] = useState<ProjectSortMode>('lastOpened');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [newProjectOpen, setNewProjectOpen] = useState(false);
  const [deleteConfirmProject, setDeleteConfirmProject] = useState<ManagedProject | null>(null);

  const filteredProjects = useMemo(
    () => sortAndFilterProjects(projects, filterQuery, sortMode),
    [projects, filterQuery, sortMode],
  );
  const selected = selectedId
    ? projects.find((project) => project.id === selectedId)
    : undefined;
  const isBusy = busy !== 'idle';

  const {
    contextMenu,
    openActionMenu,
    closeActionMenu,
  } = usePositionedActionMenu<ProjectPickerContextMenuTarget>();

  const openListContextMenu = useCallback((event: MouseEvent) => {
    openActionMenu(event, { kind: 'list' });
  }, [openActionMenu]);

  const retryPageIssue = useCallback(() => {
    if (!pageIssue) return;
    switch (pageIssue.operation) {
      case 'refresh':
        void refresh();
        break;
      case 'scan':
        void scanProjectsFromFolder();
        break;
      case 'cleanup':
        void cleanupInvalidProjects();
        break;
      case 'open':
        void openRecentProject(pageIssue.projectPath);
        break;
      case 'import':
        void importProjectFromDisk();
        break;
      case 'remove':
        void removeProject(pageIssue.projectId);
        break;
      case 'favorite':
        void toggleFavorite(pageIssue.projectId);
        break;
      case 'reveal':
        void revealProjectInExplorer(pageIssue.projectPath);
        break;
    }
  }, [
    cleanupInvalidProjects,
    importProjectFromDisk,
    openRecentProject,
    pageIssue,
    refresh,
    removeProject,
    revealProjectInExplorer,
    scanProjectsFromFolder,
    toggleFavorite,
  ]);

  const pageIssueCanRetry = pageIssue != null
    && (pageIssue.kind === 'failure' || pageIssue.operation === 'scan');

  const contextMenuSections = useMemo(
    () => buildProjectPickerContextMenuSections(contextMenu, {
      openProject: (path) => void openRecentProject(path),
      toggleFavorite: (id) => void toggleFavorite(id),
      removeProject: (id) => void removeProject(id),
      requestDeleteProjectFiles: setDeleteConfirmProject,
      revealInExplorer: (path) => void revealProjectInExplorer(path),
      newProject: () => setNewProjectOpen(true),
      importProject: () => void importProjectFromDisk(),
      scanProjects: () => void scanProjectsFromFolder(),
      cleanupProjects: () => void cleanupInvalidProjects(),
      isBusy,
    }, t),
    [
      contextMenu,
      cleanupInvalidProjects,
      importProjectFromDisk,
      isBusy,
      openRecentProject,
      removeProject,
      revealProjectInExplorer,
      scanProjectsFromFolder,
      t,
      toggleFavorite,
    ],
  );

  const handleConfirmDeleteProject = useCallback(
    (project: ManagedProject) => deleteProjectFiles(project.id),
    [deleteProjectFiles],
  );

  useEffect(() => {
    if (currentProjectId) setSelectedId(currentProjectId);
  }, [currentProjectId]);

  useEffect(() => {
    if (selectedId && !projects.some((project) => project.id === selectedId)) {
      setSelectedId(null);
    }
  }, [projects, selectedId]);

  return (
    <div className="flex h-screen min-h-0 w-full min-w-0 flex-col overflow-hidden bg-background text-foreground">
      <ProjectPickerTitleBar
        onGoEditor={() => navigate('/editor')}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      {pageIssue ? (
        <div className="shrink-0 border-b border-border bg-background p-2">
          <ProjectPickerPageIssueAlert
            issue={pageIssue}
            onDismiss={dismissPageIssue}
            onRetry={pageIssueCanRetry ? retryPageIssue : undefined}
          />
        </div>
      ) : null}
      <NewProjectModal
        open={newProjectOpen}
        onOpenChange={setNewProjectOpen}
        onCreate={createProject}
      />
      <ProjectSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      <DeleteProjectConfirmDialog
        project={deleteConfirmProject}
        onOpenChange={(open) => {
          if (!open) setDeleteConfirmProject(null);
        }}
        onConfirm={handleConfirmDeleteProject}
      />

      <div className="flex min-h-0 flex-1">
        <main className="project-picker-surface flex min-h-0 min-w-0 flex-1 flex-col">
          <ProjectPickerHero
            isBusy={isBusy}
            creating={busy === 'new'}
            importing={busy === 'import'}
            scanning={busy === 'scan'}
            onNewProject={() => setNewProjectOpen(true)}
            onImportProject={() => void importProjectFromDisk()}
            onScanProjects={() => void scanProjectsFromFolder()}
          />
          <ProjectLibrary
            projects={projects}
            filteredProjects={filteredProjects}
            selectedId={selectedId}
            currentProjectId={currentProjectId}
            filterQuery={filterQuery}
            sortMode={sortMode}
            isBusy={isBusy}
            onFilterQueryChange={setFilterQuery}
            onSortModeChange={setSortMode}
            onSelectProject={setSelectedId}
            onOpenProject={(path) => void openRecentProject(path)}
            onToggleFavorite={(id) => void toggleFavorite(id)}
            onNewProject={() => setNewProjectOpen(true)}
            onImportProject={() => void importProjectFromDisk()}
            onListContextMenu={openListContextMenu}
            onProjectContextMenu={(event, project) => {
              setSelectedId(project.id);
              openActionMenu(event, { kind: 'project', project });
            }}
          />
        </main>
        <ProjectPickerActionPanel
          selected={selected}
          currentProjectId={currentProjectId}
          isBusy={isBusy}
          cleaningUp={busy === 'cleanup'}
          onOpenProject={(path) => void openRecentProject(path)}
          onRevealProject={(path) => void revealProjectInExplorer(path)}
          onToggleFavorite={(id) => void toggleFavorite(id)}
          onRemoveProject={(id) => void removeProject(id)}
          onDeleteProject={setDeleteConfirmProject}
          onCleanupProjects={() => void cleanupInvalidProjects()}
        />
      </div>

      {contextMenu ? (
        <ActionMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={contextMenuSections}
          onClose={closeActionMenu}
        />
      ) : null}
    </div>
  );
}
