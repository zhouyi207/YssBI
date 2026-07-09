import { memo, useContext } from 'react';
import Canvas from './Canvas';
import { useEditorGroup } from '@/features/application/editor';
import { GroupContext } from '@/features/core/editor';
import { WatermarkView } from '../overlays/WatermarkView';
import { InactiveEditorGroupPlaceholder } from '../overlays/InactiveEditorGroupPlaceholder';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useShallow } from 'zustand/react/shallow';
import { CanvasDropZone } from './CanvasDropZone';

/**
 * 图形编辑器主组件
 * 负责渲染无限画布 (Canvas) 或空状态
 */
export const GraphEditor = memo(function GraphEditor() {
    const nodeId = useContext(GroupContext) as string | null;
    const isActiveGroup = useLayoutStore((s) => s.activeEditorGroupId === nodeId);
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
            aria-hidden={!isActiveGroup}
        >
            <div className="flex-1 relative overflow-hidden">
                <CanvasDropZone groupId={nodeId ?? 'default_editor'}>
                    {!isActiveGroup ? (
                        resolvedTabId ? <InactiveEditorGroupPlaceholder /> : null
                    ) : resolvedTabId ? (
                        <Canvas />
                    ) : (
                        <WatermarkView />
                    )}
                </CanvasDropZone>
            </div>
        </div>
    );
});

export default GraphEditor;
