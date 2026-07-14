import { useRef, Fragment, useMemo, memo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { inferPanelPosition } from '@/features/core/layout/panelPartLayout';
import { readEditorAreaMaximizedGroupId } from '@/features/core/layout/editorGridLayout';
import {
  resolveWorkbenchDropSurfaceFlags,
  WORKBENCH_CHROME_PART_ATTR,
  WORKBENCH_EDITOR_SURFACE_ATTR,
} from '@/features/core/layout/workbenchSidebarDropSurface';
import { Sash } from './Sash';
import { layoutNodeFlexStyle } from './sashResizeLogic';
import { viewRegistry } from './viewRegistry';
import { GroupContext } from '@/features/core/editor';
import { useEditorGroupTabStrip } from '@/features/core/editor/hooks/useEditorGroupTabStrip';
import { TabBar } from '../Layout/TabBar';
import { activateEditorGroup } from '@/features/application/editor/switchEditorTab';
import { useEditorDropPreviewStore } from '@/features/application/editor/editorDropPreviewStore';

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

    const collapsedById = useLayoutStore(useShallow((state) => {
        const map: Record<string, boolean> = {};
        for (const id of childrenIds) {
            const child = state.nodes[id];
            map[id] =
                child?.data?.visible === false
                || child?.data?.groupMaximizedHidden === true;
        }
        return map;
    }));

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
                const nextId = childrenIds[index + 1];
                const showSash =
                    nextId != null
                    && !collapsedById[childId]
                    && !collapsedById[nextId];

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
    const nodes = useLayoutStore((state) => state.nodes);
    const panelMaximized = useLayoutStore((s) => s.nodes.panel?.data?.maximized === true);
    const panelPosition = useLayoutStore((s) => inferPanelPosition(s.nodes));
    const maximizedEditorGroupId = useLayoutStore((s) => readEditorAreaMaximizedGroupId(s.nodes));
    const dropSurfaceFlags = useMemo(
        () => resolveWorkbenchDropSurfaceFlags(nodeId, nodes),
        [nodeId, nodes],
    );
    const style = useMemo(
        () => layoutNodeFlexStyle(node, { panelMaximized, panelPosition, maximizedEditorGroupId }),
        [node, panelMaximized, panelPosition, maximizedEditorGroupId],
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
            {...(dropSurfaceFlags.chromePart
              ? { [WORKBENCH_CHROME_PART_ATTR]: dropSurfaceFlags.chromePart }
              : {})}
            {...(dropSurfaceFlags.editorSurface
              ? { [WORKBENCH_EDITOR_SURFACE_ATTR]: '' }
              : {})}
        >
            {maximizedHidden ? (
                <div
                    className="h-full w-full"
                    data-editor-group-maximized-placeholder={nodeId}
                    aria-hidden="true"
                />
            ) : (!hidden || keepAlive) && (
                <div
                    className={`h-full w-full ${hidden && keepAlive ? 'hidden' : ''}`}
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
    const defaultComponent = useLayoutStore((s) => s.nodes[nodeId]?.data?.component ?? '');
    const isFixed = useLayoutStore((s) => !!s.nodes[nodeId]?.data?.isFixed);
    const tabSlice = useEditorTabStore(useShallow((state) => {
        const placement = state.placements[nodeId];
        const activeTabId = placement?.activeTabId;
        const activeTab = activeTabId ? state.registry[activeTabId] : null;
        return {
            hasTabs: (placement?.tabIds.length ?? 0) > 0,
            activeComponentName: activeTab?.component ?? '',
        };
    }));

    const activeComponentName = tabSlice.activeComponentName || defaultComponent;

    const ActiveComponent = useMemo(() => {
        return activeComponentName ? viewRegistry.get(activeComponentName) : null;
    }, [activeComponentName]);

    if (!defaultComponent && !tabSlice.hasTabs) return null;

    return (
        <EditorGroupFocusShell
            nodeId={nodeId}
            isFixed={isFixed}
            hasTabs={tabSlice.hasTabs}
        >
            {tabSlice.hasTabs ? (
                <div className="flex-none flex items-center bg-[var(--workbench-bg)] select-none">
                    <EditorGroupTabStrip layoutNodeId={nodeId} />
                </div>
            ) : null}

            <EditorGroupContent nodeId={nodeId}>
                {ActiveComponent ? (
                    <ActiveComponent />
                ) : (
                    <div className="p-4 italic text-muted-foreground">No content</div>
                )}
            </EditorGroupContent>
        </EditorGroupFocusShell>
    );
};

/** Isolated focus-ring subscription — switching active group does not re-render editor content. */
const EditorGroupFocusShell = memo(function EditorGroupFocusShell({
    nodeId,
    isFixed,
    hasTabs,
    children,
}: {
    nodeId: string;
    isFixed: boolean;
    hasTabs: boolean;
    children: React.ReactNode;
}) {
    const isActive = useLayoutStore((s) => s.activeEditorGroupId === nodeId);
    const isDragTarget = useEditorDropPreviewStore((s) => s.preview?.targetGroupId === nodeId);

    return (
        <GroupContext.Provider value={nodeId}>
            <div
                onPointerDown={(e) => {
                    if (e.button !== 0) return;
                    if ((e.target as HTMLElement).closest(
                        '[data-tab-id], [data-tab-strip], [data-tabbar-drop], [data-editor-group-actions]',
                    )) return;
                    void activateEditorGroup(nodeId);
                }}
                className={`w-full h-full relative flex flex-col overflow-hidden bg-[var(--workbench-bg)] transition-shadow duration-200 ${isFixed ? 'z-20' : ''} ${isDragTarget ? 'ring-1 ring-inset ring-primary/40' : ''} ${isActive && (hasTabs || !isFixed) ? 'z-10 ring-1 ring-inset ring-[var(--accent-color)]/30 shadow-[0_0_15px_rgba(0,0,0,0.3)]' : ''}`}
                id={`layout-node-${nodeId}`}
            >
                {children}
            </div>
        </GroupContext.Provider>
    );
});

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

/** Editor body — VS Code drop overlay uses pointer hit-test on `data-editor-content`. */
const EditorGroupContent = ({ nodeId, children }: { nodeId: string; children: React.ReactNode }) => {
    return (
        <div
            data-editor-content={nodeId}
            className="relative flex min-h-0 flex-1 flex-col"
        >
            {children}
        </div>
    );
};

export default LayoutNodeRenderer;
