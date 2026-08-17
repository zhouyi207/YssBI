// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { LoadStatus } from '@/shared/types/ui';
import type { StatusBarRenderContext } from '@/features/core/statusBar';
import { projectStatusLabel } from './useStatusBarItems';

const translations: Record<string, string> = {
  'bottomBar.projectError': 'Project unavailable',
  'bottomBar.loadingProject': 'Loading project',
  'common.incidentId': 'Incident ID',
  'common.ready': 'Ready',
  'common.idle': 'Idle',
};

const t = ((key: string) => translations[key] ?? key) as StatusBarRenderContext['t'];

describe('projectStatusLabel', () => {
  it('renders localized project text, code, and optional incident ID', () => {
    expect(projectStatusLabel(
      LoadStatus.Error,
      { code: 'project_io_failed', incidentId: 'incident-project-42' },
      t,
    )).toBe('Project unavailable [project_io_failed] · Incident ID: incident-project-42');
  });

  it('renders the stable code without inventing an incident ID', () => {
    expect(projectStatusLabel(
      LoadStatus.Error,
      { code: 'project_load_contract_error', incidentId: null },
      t,
    )).toBe('Project unavailable [project_load_contract_error]');
  });
});
