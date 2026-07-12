import { useRef, Fragment, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { inferPanelPosition } from '@/features/core/layout/panelPartLayout';
import { useSidebarDragStore } from '@/features/core/sidebarDrag';
import { Sash } from './Sash';
import { layoutNodeFlexStyle } from './sashResizeLogic';
import { viewRegistry } from './viewRegistry';
import { useDroppable } from '@dnd-kit/core';
import { GroupContext } from '@/features/core/editor';
import { useEditorGroupTabStrip } from '@/features/core/editor/hooks/useEditorGroupTabStrip';
import { TabBar } from '../Layout/TabBar';
import { DROP_TYPES } from '@/features/core/dnd';
import { activateEditorGroup } from '@/features/application/editor/switchEditorTab';

/**
 * 布局引擎核心渲染器
 * 负责根据节点类型（容器或组件）递归分发渲染任务
 */
export const LayoutNodeRenderer = ({ nodeId }: { nodeId: string }) => {
    const nodeKind = useLayoutStore(useShallow((state) => {
        const node = state.nodes[nodeId];
        if (!node) return null;
        if (node.type === 'component') return 'component' as const;
        return {
            orientation: node.type === 'row' ? 'row' as const : 'col' as const,
            childrenIds: node.children ?? [],
        };
    }));

    if (!nodeKind) return null;

    if (nodeKind === 'component') {
        return <LeafNodeRenderer nodeId={nodeId} />;
    }

    return (
        <ContainerNodeRenderer
            nodeId={nodeId}
            orientation={nodeKind.orientation}
            childrenIds={nodeKind.childrenIds}
        />
    );
};

/**
 * 容器节点渲染器 (Row/Col)
 * 核心逻辑：自动在子节点之间插入可调节大小的 Sash
 */
const ContainerNodeRenderer = ({
    orientation,
    childrenIds,
}: {
    nodeId: string;
    orientation: 'row' | 'col';
    childrenIds: string[];
}) => {
    const isRow = orientation === 'row';

    // 维护子节点 DOM 引用，供 Sash 调节大小使用
    const childrenRefs = useRef<Map<string, HTMLDivElement>>(new Map());

    const proxyRefs = useMemo(() => new Map<string, React.RefObject<HTMLDivElement | null>>(), []);

    const getChildRef = (id: string) => {
        if (!proxyRefs.has(id)) {
            proxyRefs.set(id, {
                get current() {
                    return childrenRefs.current.get(id) || null;
                },
            } as React.RefObject<HTMLDivElement | null>);
        }
        return proxyRefs.get(id)!;
    };

    return (
        <div className={`flex w-full h-full overflow-hidden ${isRow ? 'flex-row' : 'flex-col'}`}>
            {childrenIds.map((childId, index) => {
                const showSash = index < childrenIds.length - 1;

                return (
                    <Fragment key={childId}>
                        <ChildWrapper
                            nodeId={childId}
                            setRef={(el) => {
                                if (el) childrenRefs.current.set(childId, el);
                                else childrenRefs.current.delete(childId);
                            }}
                        />

                        {showSash && (
                            <Sash
                                orientation={orientation}
                                beforeRef={getChildRef(childId)}
                                afterRef={getChildRef(childrenIds[index + 1])}
                                beforeNodeId={childId}
                                afterNodeId={childrenIds[index + 1]}
                            />
                        )}
                    </Fragment>
                );
            })}
        </div>
    );
};

/**
 * 子节点包裹层
 * 独立订阅节点状态，确保尺寸(pixelSize/size)变化时能实时触发重绘
 */
const ChildWrapper = ({ nodeId, setRef }: { nodeId: string, setRef: (el: HTMLDivElement | null) => void }) => {
    const node = useLayoutStore(useShallow((state) => state.nodes[nodeId]));
    const panelMaximized = useLayoutStore((s) => s.nodes.panel?.data?.maximized === true);
    const panelPosition = useLayoutStore((s) => inferPanelPosition(s.nodes));
    const style = useMemo(
        () => layoutNodeFlexStyle(node, { panelMaximized, panelPosition }),
        [node, panelMaximized, panelPosition],
    );

    if (!node) return null;

    const hidden = node.data?.visible === false;
    const keepAlive = node.data?.isFixed === true;
    const maximizedHidden = node.data?.groupMaximizedHidden === true;

    return (
        <div
            ref={setRef}
            className="layout-split-view relative min-h-0 min-w-0"
            style={style}
        >
            {maximizedHidden ? (
                <div
                    className="h-full w-full"
                    data-editor-group-maximized-placeholder={nodeId}
                    aria-hidden="true"
                />
            ) : (!hidden || keepAlive) && (
                <div
                    className={`h-full w-full ${hidden && keepAlive ? 'invisible pointer-events-none' : ''}`}
                    aria-hidden={hidden || undefined}
                >
                    <LayoutNodeRenderer nodeId={nodeId} />
                </div>
            )}
        </div>
    );
};

/**
 * 叶子节点渲染器
 * 负责渲染具体的业务组件以及处理 DND 区域
 */
const LeafNodeRenderer = ({ nodeId }: { nodeId: string }) => {
    const activeEditorGroupId = useLayoutStore(s => s.activeEditorGroupId);

    const leaf = useLayoutStore(useShallow((s) => {
        const node = s.nodes[nodeId];
        if (!node) return null;
        const tabs = node.data?.tabs;
        const activeTabId = node.data?.activeTabId;
        const activeTab = tabs?.find((tab) => tab.id === activeTabId);
        const hasTabs = (tabs?.length ?? 0) > 0;
        return {
            hasTabs,
            activeComponentName: activeTab?.component ?? node.data?.component ?? '',
            isFixed: !!node.data?.isFixed,
        };
    }));

    const isActive = activeEditorGroupId === nodeId;

    const ActiveComponent = useMemo(() => {
        if (!leaf) return null;
        return leaf.activeComponentName ? viewRegistry.get(leaf.activeComponentName) : null;
    }, [leaf?.activeComponentName]);

    if (!leaf) return null;

    return (
        <GroupContext.Provider value={nodeId}>
            <div
                onClick={() => void activateEditorGroup(nodeId)}
                className={`w-full h-full relative flex flex-col overflow-hidden bg-[var(--workbench-bg)] transition-shadow duration-200 ${leaf.isFixed ? 'z-20' : ''} ${isActive && (leaf.hasTabs || !leaf.isFixed) ? 'z-10 ring-1 ring-inset ring-[var(--accent-color)]/30 shadow-[0_0_15px_rgba(0,0,0,0.3)]' : ''}`}
                id={`layout-node-${nodeId}`}
            >
                {leaf.hasTabs ? (
                    <div className="flex-none flex items-center bg-[var(--workbench-bg)] select-none">
                        <EditorGroupTabStrip layoutNodeId={nodeId} />
                    </div>
                ) : null}

                <div className="flex-1 relative min-h-0" data-editor-content={nodeId}>
                    {ActiveComponent ? (
                        <ActiveComponent />
                    ) : (
                        <div className="p-4 italic text-muted-foreground">No content</div>
                    )}

                    {!leaf.isFixed && <DropZoneOverlay nodeId={nodeId} />}
                </div>
            </div>
        </GroupContext.Provider>
    );
};

/** Tab strip with narrow layout subscription — avoids re-rendering editor content on tab chrome changes. */
const EditorGroupTabStrip = ({ layoutNodeId }: { layoutNodeId: string }) => {
    const { tabs, activeTabId } = useEditorGroupTabStrip(layoutNodeId);
    if (!tabs.length) return null;
    return (
        <TabBar
            layoutNodeId={layoutNodeId}
            tabs={tabs}
            activeTabId={activeTabId}
        />
    );
};

/**
 * 拖拽停靠区域覆盖层 (DND Drop Zones)
 * 仅在拖拽布局节点或 Tab 时显示。
 * Sidebar 拖拽由 CanvasDropZone / sidebar folder drop target 单独处理。
 */
const DropZoneOverlay = ({ nodeId }: { nodeId: string }) => {
    const isDragging = useLayoutStore(s => s.isDragging);
    const isSidebarDrag = useSidebarDragStore(s => !!s.activeDrag);
    if (!isDragging || isSidebarDrag) return null;

    return (
        <div className="absolute inset-0 pointer-events-none z-10 flex flex-col">
            <DroppableZone nodeId={nodeId} zone="top" className="h-[20%] w-full pointer-events-auto" />
            <div className="flex-1 flex">
                <DroppableZone nodeId={nodeId} zone="left" className="h-full w-[20%] pointer-events-auto" />
                <DroppableZone nodeId={nodeId} zone="center" className="flex-1 h-full pointer-events-auto" />
                <DroppableZone nodeId={nodeId} zone="right" className="h-full w-[20%] pointer-events-auto" />
            </div>
            <DroppableZone nodeId={nodeId} zone="bottom" className="h-[20%] w-full pointer-events-auto" />
        </div>
    );
};

const DroppableZone = ({ nodeId, zone, className }: { nodeId: string, zone: string, className: string }) => {
    const { setNodeRef } = useDroppable({
        id: `${nodeId}-${zone}`,
        data: { dropType: DROP_TYPES.LAYOUT_REGION, dropPosition: zone, targetNodeId: nodeId }
    });

    return (
        <div
            id={`${nodeId}-${zone}`}
            ref={setNodeRef}
            className={className}
        />
    );
};

export default LayoutNodeRenderer;
