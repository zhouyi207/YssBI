import { memo, useContext } from 'react';
import Canvas from './Canvas';
import { useEditorGroup, useIsActiveEditorGroup } from '@/features/application/editor';
import { GroupContext } from '@/features/core/editor';
import { WatermarkView } from '../overlays/WatermarkView';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useShallow } from 'zustand/react/shallow';
import { CanvasDropZone } from './CanvasDropZone';

/**
 * Graph editor shell per editor group.
 * Always renders the active tab's canvas; inactive groups use preview mode (visible, non-interactive).
 */
export const GraphEditor = memo(function GraphEditor() {
    const nodeId = useContext(GroupContext) as string | null;
    const isActiveGroup = useIsActiveEditorGroup(nodeId);
    const { activeTabId: contextActiveTabId } = useEditorGroup();

    const { hasTabs, activeTabId } = useLayoutStore(useShallow((s) => {
        if (!nodeId) {
            return { hasTabs: false, activeTabId: contextActiveTabId as string | null };
        }
        const node = s.nodes[nodeId];
        const tabsLen = node?.data?.tabs?.length ?? 0;
        return {
            hasTabs: tabsLen > 0,
            activeTabId: tabsLen > 0 ? (node?.data?.activeTabId ?? null) : null,
        };
    }));

    const resolvedTabId = nodeId
        ? (hasTabs ? activeTabId : null)
        : contextActiveTabId;

    return (
        <div
            className={`flex flex-col w-full h-full overflow-hidden ${isActiveGroup ? '' : 'pointer-events-none'}`}
            aria-hidden={!isActiveGroup || undefined}
        >
            <div className="flex-1 relative overflow-hidden">
                <CanvasDropZone groupId={nodeId ?? 'default_editor'} interactive={isActiveGroup}>
                    {resolvedTabId ? (
                        <Canvas interactive={isActiveGroup} />
                    ) : (
                        <WatermarkView />
                    )}
                </CanvasDropZone>
            </div>
        </div>
    );
});

export default GraphEditor;
