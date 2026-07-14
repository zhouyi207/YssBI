import { memo, useContext } from 'react';
import Canvas from './Canvas';
import { useIsActiveEditorGroup } from '@/features/application/editor';
import { GroupContext, useEditorGroupWorkspace } from '@/features/core/editor';
import { WatermarkView } from '../overlays/WatermarkView';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { CanvasDropZone } from './CanvasDropZone';

/**
 * Graph editor shell per editor group.
 * Always renders the active tab's canvas; inactive groups use preview mode (visible, non-interactive).
 */
export const GraphEditor = memo(function GraphEditor() {
    const nodeId = useContext(GroupContext) as string | null;
    const isActiveGroup = useIsActiveEditorGroup(nodeId);
    const { activeTabId, tabs } = useEditorGroupWorkspace();

    const resolvedTabId = tabs.length > 0 ? activeTabId : null;

    return (
        <div className="flex flex-col w-full h-full overflow-hidden">
            <div className="flex-1 relative overflow-hidden">
                <CanvasDropZone groupId={nodeId ?? DEFAULT_EDITOR_GROUP_ID} interactive={isActiveGroup}>
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
