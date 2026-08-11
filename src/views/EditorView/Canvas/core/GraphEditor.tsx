import { memo, useContext } from 'react';
import Canvas from './Canvas';
import { useIsActiveEditorGroup } from '@/features/application/editor';
import { GroupContext, useEditorGroupWorkspace } from '@/features/core/editor';
import { WatermarkView } from '../overlays/WatermarkView';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { CanvasDropZone } from './CanvasDropZone';
import { useGraphDataStore, useProjectIOStore } from '@/features/core/dataStore';
import { resourceKey, useDocumentStateStore } from '@/features/core/resource';

/**
 * Graph editor shell per editor group.
 * Always renders the active tab's canvas; inactive groups use preview mode (visible, non-interactive).
 */
export const GraphEditor = memo(function GraphEditor() {
    const nodeId = useContext(GroupContext) as string | null;
    const isActiveGroup = useIsActiveEditorGroup(nodeId);
    const { activeTabId, tabs } = useEditorGroupWorkspace();

    const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
    const graphKind = activeTab?.type === 'event' || activeTab?.type === 'function'
        ? activeTab.type
        : null;
    const graphLoadStatus = useProjectIOStore((state) => (
        activeTabId ? state.graphLoadStatus[activeTabId] : undefined
    ));
    const graphProjectionReady = useGraphDataStore((state) => (
        activeTabId ? state.hasGraph(activeTabId) : false
    ));
    const graphDocument = useDocumentStateStore((state) => (
        activeTabId && graphKind
            ? state.documents[resourceKey({ id: activeTabId, kind: graphKind })]
            : undefined
    ));
    const resolvedTabId = tabs.length > 0 ? activeTabId : null;
    const graphReady = resolvedTabId !== null
        && graphKind !== null
        && graphProjectionReady
        && graphDocument?.loaded === true
        && graphDocument.stale === false
        && graphDocument.conflict === false
        && graphLoadStatus !== 'loading'
        && graphLoadStatus !== 'error';
    const graphUnavailable = graphLoadStatus === 'error' || graphDocument?.conflict === true;

    return (
        <div className="flex flex-col w-full h-full min-h-0 min-w-0 overflow-hidden">
            <div className="flex-1 relative min-h-0 min-w-0 overflow-hidden">
                <CanvasDropZone groupId={nodeId ?? DEFAULT_EDITOR_GROUP_ID} interactive={isActiveGroup}>
                    {resolvedTabId ? (
                        graphReady
                            ? <Canvas interactive={isActiveGroup} />
                            : graphUnavailable
                                ? <div className="absolute inset-0" role="alert" data-graph-load-error />
                                : <div className="absolute inset-0" aria-busy="true" data-graph-loading />
                    ) : (
                        <WatermarkView />
                    )}
                </CanvasDropZone>
            </div>
        </div>
    );
});

export default GraphEditor;
