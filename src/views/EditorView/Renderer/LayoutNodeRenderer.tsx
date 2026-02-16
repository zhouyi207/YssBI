import { useRef, Fragment, useMemo } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { Sash } from './Sash';
import { viewRegistry } from './viewRegistry';
import { LayoutNode } from '@/shared/types/ui';
import { useDraggable, useDroppable } from '@dnd-kit/core';
import { GroupContext } from '@/features/application/editor/core/hooks/useEditorGroup';
import { TabBar } from '../Layout/TabBar';

/**
 * 布局引擎核心渲染器
 * 负责根据节点类型（容器或组件）递归分发渲染任务
 */
export const LayoutNodeRenderer = ({ nodeId }: { nodeId: string }) => {
    const node = useLayoutStore((state) => state.nodes[nodeId]);

    if (!node) return null;

    // 1. 如果是叶子组件节点
    if (node.type === 'component') {
        return <LeafNodeRenderer node={node} />;
    }

    // 2. 如果是容器节点（row 或 col）
    return <ContainerNodeRenderer node={node} />;
};

/**
 * 容器节点渲染器 (Row/Col)
 * 核心逻辑：自动在子节点之间插入可调节大小的 Sash
 */
const ContainerNodeRenderer = ({ node }: { node: LayoutNode }) => {
    const childrenIds = node.children || [];
    const orientation = node.type === 'row' ? 'row' : 'col';
    const isRow = orientation === 'row';

    // 维护子节点 DOM 引用，供 Sash 调节大小使用
    const childrenRefs = useRef<Map<string, HTMLDivElement>>(new Map());

    // 使用 useMemo 确保代理 Ref 对象在节点 ID 不变时保持稳定
    const beforeRefs = useMemo(() => new Map<string, React.RefObject<HTMLDivElement | null>>(), []);
    const afterRefs = useMemo(() => new Map<string, React.RefObject<HTMLDivElement | null>>(), []);

    const getProxyRef = (id: string, map: Map<string, React.RefObject<HTMLDivElement | null>>) => {
        if (!map.has(id)) {
            map.set(id, {
                get current() {
                    return childrenRefs.current.get(id) || null;
                }
            } as React.RefObject<HTMLDivElement | null>);
        }
        return map.get(id)!;
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
                                index={index}
                                beforeRef={getProxyRef(childId, beforeRefs)}
                                afterRef={getProxyRef(childrenIds[index + 1], afterRefs)}
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
    const node = useLayoutStore((state) => state.nodes[nodeId]);

    if (!node || node.data?.visible === false) return null;

    const style = node.pixelSize !== undefined
        ? { flex: `0 0 ${node.pixelSize}px` }
        : { flex: `${node.size ?? 1} 1 0px` };

    return (
        <div
            ref={setRef}
            className="relative min-w-0 min-h-0"
            style={style}
        >
            <LayoutNodeRenderer nodeId={nodeId} />
        </div>
    );
};

/**
 * 叶子节点渲染器
 * 负责渲染具体的业务组件以及处理 DND 区域
 */
const LeafNodeRenderer = ({ node }: { node: LayoutNode }) => {
    const activeGroupId = useLayoutStore(s => s.activeGroupId);
    const setActiveGroup = useLayoutStore(s => s.setActiveGroup);
    const isActive = activeGroupId === node.id;

    // 1. 获取 Tab 数据
    const tabs = node?.data?.tabs || [];
    const activeTabId = node?.data?.activeTabId;
    const hasTabs = tabs.length > 0;

    // 2. 确定当前要渲染的业务组件
    const ActiveComponent = useMemo(() => {
        if (!node) return null;
        let componentName = node.data?.component || ''; // 默认使用基础组件
        
        if (hasTabs) {
            const activeTab = tabs.find(t => t?.id === activeTabId);
            if (activeTab) {
                componentName = activeTab.component;
            }
        }
        
        return componentName ? viewRegistry.get(componentName) : null;
    }, [hasTabs, tabs, activeTabId, node?.data?.component]);

    // DND 拖拽支持 - 只有非固定节点可以拖动
    const isFixed = !!node.data?.isFixed;

    const { attributes: _attributes, listeners: _listeners, setNodeRef: setDragRef, transform, isDragging } = useDraggable({
        id: node.id,
        data: { type: 'leaf', node },
        disabled: isFixed // 禁用固定节点的拖拽功能
    });

    const style = transform ? {
        transform: `translate3d(${transform.x}px, ${transform.y}px, 0)`,
        zIndex: isDragging ? 100 : 'auto',
        opacity: isDragging ? 0.5 : 1
    } : {
        opacity: isDragging ? 0.5 : 1
    };

    return (
        <GroupContext.Provider value={node.id}>
            <div
                ref={setDragRef}
                style={style}
                onClick={() => setActiveGroup(node.id)}
                className={`w-full h-full relative flex flex-col overflow-hidden bg-[var(--workbench-bg)] transition-shadow duration-200 ${isActive && (hasTabs || !isFixed) ? 'z-10 ring-1 ring-inset ring-[var(--accent-color)]/30 shadow-[0_0_15px_rgba(0,0,0,0.3)]' : ''}`}
                id={`layout-node-${node.id}`}
            >
                {/* 统一 Header 区域 */}
                {hasTabs ? (
                    <div className="flex-none flex items-center bg-[var(--workbench-bg)] select-none">
                        <TabBar
                            layoutNodeId={node.id}
                            tabs={tabs}
                            activeTabId={activeTabId}
                        />
                    </div>
                ) : !isFixed ? (
                    // <div
                    //     {...attributes}
                    //     {...listeners}
                    //     className="flex-none h-9 px-3 flex items-center bg-[var(--workbench-bg)] select-none text-[11px] font-medium text-gray-400 uppercase tracking-wider cursor-grab active:cursor-grabbing"
                    // >
                    //     {node.data?.title || node.id}
                    // </div>
                    <></>
                ) : null}

                {/* 内容区域 */}
                <div className="flex-1 relative min-h-0">
                    {ActiveComponent ? (
                        <ActiveComponent />
                    ) : (
                        <div className="p-4 text-gray-500 italic">No content</div>
                    )}

                    {/* 拖拽停靠区域覆盖层 - 固定节点不允许作为停靠目标 */}
                    {!isFixed && <DropZoneOverlay nodeId={node.id} />}
                </div>
            </div>
        </GroupContext.Provider>
    );
};

/**
 * 拖拽停靠区域覆盖层 (DND Drop Zones)
 */
const DropZoneOverlay = ({ nodeId }: { nodeId: string }) => {
    const isDragging = useLayoutStore(s => s.isDragging);
    if (!isDragging) return null;

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
    const { setNodeRef, isOver } = useDroppable({
        id: `${nodeId}-${zone}`,
        data: { dropPosition: zone, targetNodeId: nodeId }
    });

    return (
        <div
            id={`${nodeId}-${zone}`}
            ref={setNodeRef}
            className={`${className} ${isOver ? 'bg-blue-500/20 transition-colors' : ''}`}
        />
    );
};

export default LayoutNodeRenderer;
