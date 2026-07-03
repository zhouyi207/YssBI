import { useContext } from 'react';
import Canvas from './Canvas';
import { useEditorGroup } from '@/features/application/editor';
import { GroupContext } from '@/features/core/editor';
import { WatermarkView } from '../overlays/WatermarkView';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { CanvasDropZone } from './CanvasDropZone';

/**
 * 图形编辑器主组件
 * 负责渲染无限画布 (Canvas) 或空状态
 */
export const GraphEditor = () => {
    const nodeId = useContext(GroupContext) as string | null;
    const node = useLayoutStore(s => nodeId ? s.nodes[nodeId] : null);
    const { activeTabId: contextActiveTabId } = useEditorGroup();

    // 确定当前组是否有标签
    const hasTabs = (node?.data?.tabs?.length ?? 0) > 0;
    
    // 如果是布局节点且没有标签，强制设为 null 以显示空状态
    // 否则优先使用节点的 activeTabId，最后才回退到 Context
    const activeTabId = nodeId 
        ? (hasTabs ? node?.data?.activeTabId : null)
        : contextActiveTabId;

    return (
        <div className="flex flex-col w-full h-full overflow-hidden">
            {/* 主内容区域 - CanvasDropZone 使侧边栏可拖放到画布 */}
            <div className="flex-1 relative overflow-hidden">
                <CanvasDropZone groupId={nodeId ?? "default_editor"}>
                    {activeTabId ? (
                        <Canvas />
                    ) : (
                        <WatermarkView />
                    )}
                </CanvasDropZone>
            </div>
        </div>
    );
};

export default GraphEditor;
