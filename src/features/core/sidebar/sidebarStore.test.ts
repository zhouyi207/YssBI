import { beforeEach, describe, expect, it } from 'vitest';
import { SIDEBAR_SECTION_DEFAULTS, useSidebarStore } from './sidebarStore';
import { mergeExpandedSections, resolveSectionExpanded } from './sidebarSectionState';

describe('sidebarStore section expand', () => {
  beforeEach(() => {
    useSidebarStore.setState({
      expandedSections: { ...SIDEBAR_SECTION_DEFAULTS },
      expandedGroups: {},
    });
  });

  it('resolveSectionExpanded falls back to defaults for unknown keys', () => {
    expect(resolveSectionExpanded({}, 'graphsEvent')).toBe(true);
    expect(resolveSectionExpanded({ graphsEvent: false }, 'graphsEvent')).toBe(false);
  });

  it('toggleSection toggles independently without closing siblings', () => {
    expect(useSidebarStore.getState().isSectionExpanded('graphsEvent')).toBe(true);
    expect(useSidebarStore.getState().isSectionExpanded('graphsFunction')).toBe(false);

    useSidebarStore.getState().toggleSection('graphsFunction');
    expect(useSidebarStore.getState().isSectionExpanded('graphsEvent')).toBe(true);
    expect(useSidebarStore.getState().isSectionExpanded('graphsFunction')).toBe(true);

    useSidebarStore.getState().toggleSection('graphsEvent');
    expect(useSidebarStore.getState().isSectionExpanded('graphsEvent')).toBe(false);
    expect(useSidebarStore.getState().isSectionExpanded('graphsFunction')).toBe(true);
  });

  it('mergeExpandedSections preserves multiple expanded siblings', () => {
    const merged = mergeExpandedSections({
      graphsEvent: true,
      graphsFunction: true,
    });
    expect(merged.graphsEvent).toBe(true);
    expect(merged.graphsFunction).toBe(true);
  });
});
