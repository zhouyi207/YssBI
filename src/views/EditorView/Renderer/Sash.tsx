
import React, { useRef, useEffect } from 'react';
import { LayoutDirection } from '@/shared/types/ui';
import { useLayoutStore } from '@/features/core/layout/layoutStore';


interface SashProps {
    orientation: LayoutDirection; // 'row' means the container is a row, so sash is vertical? No, usually typical split view terminology:
    // If container is 'row', children are side-by-side, so Sash is a vertical divider.
    // If container is 'col', children are stacked, so Sash is a horizontal divider.
    // Let's stick to: orientation is the direction of the parent split view.

    index: number; // Index of the sash (0 means between child 0 and 1)

    // We pass refs to the elements before and after this sash for direct manipulation
    beforeRef: React.RefObject<HTMLDivElement | null>;
    afterRef: React.RefObject<HTMLDivElement | null>;

    // Node IDs to update in store after drag
    beforeNodeId: string;
    afterNodeId: string;
}

export const Sash: React.FC<SashProps> = ({
    orientation,
    beforeRef,
    afterRef,
    beforeNodeId,
    afterNodeId
}) => {
    const sashRef = useRef<HTMLDivElement>(null);
    const isDragging = useRef(false);
    const startPos = useRef(0);
    const startSizes = useRef<{ before: number; after: number } | null>(null);

    // Store actions
    // We will need an action to update size. For now we assume we might dispatch later.
    // const updateNodeSize = useLayoutStore(s => s.updateNodeSize); // TODO: implement this in store

    useEffect(() => {
        const sash = sashRef.current;
        if (!sash) return;

        const handleMouseDown = (e: MouseEvent) => {
            e.preventDefault();
            e.stopPropagation();

            const beforeEl = beforeRef.current;
            const afterEl = afterRef.current;

            if (!beforeEl || !afterEl) return;

            isDragging.current = true;
            
            // Performance optimization: add dragging class to body
            document.body.classList.add('layout-sash-dragging');
            document.body.classList.add(orientation === 'row' ? 'col-resize' : 'row-resize');

            if (sashRef.current) {
                sashRef.current.classList.add('active');
            }

            // Record start position
            startPos.current = orientation === 'row' ? e.clientX : e.clientY;

            // Record start sizes (getBoundingClientRect includes borders/padding)
            const beforeRect = beforeEl.getBoundingClientRect();
            const afterRect = afterEl.getBoundingClientRect();

            startSizes.current = {
                before: orientation === 'row' ? beforeRect.width : beforeRect.height,
                after: orientation === 'row' ? afterRect.width : afterRect.height
            };

            // Add window listeners
            window.addEventListener('mousemove', handleMouseMove);
            window.addEventListener('mouseup', handleMouseUp);
        };

        const handleMouseMove = (e: MouseEvent) => {
            if (!isDragging.current || !startSizes.current) return;

            const currentPos = orientation === 'row' ? e.clientX : e.clientY;
            const delta = currentPos - startPos.current;

            const { nodes, resizeNode, updateNode } = useLayoutStore.getState();
            const beforeNode = nodes[beforeNodeId];
            const afterNode = nodes[afterNodeId];

            if (beforeNode?.data?.visible === false) {
                const restored = { ...beforeNode.data, visible: true };
                if (!restored.currentTab && restored.component === 'Sidebar') restored.currentTab = 'graphs';
                updateNode(beforeNodeId, { data: restored });
            }
            if (afterNode?.data?.visible === false) {
                const restored = { ...afterNode.data, visible: true };
                updateNode(afterNodeId, { data: restored });
            }

            if (beforeNode?.pixelSize !== undefined) {
                const newSize = Math.max(beforeNode.minSize ?? 0, startSizes.current.before + delta);
                resizeNode(beforeNodeId, newSize);
            } else if (afterNode?.pixelSize !== undefined) {
                const newSize = Math.max(afterNode.minSize ?? 0, startSizes.current.after - delta);
                resizeNode(afterNodeId, newSize);
            } else {
                const newSize = Math.max(beforeNode?.minSize ?? 0, startSizes.current.before + delta);
                resizeNode(beforeNodeId, newSize);
            }
        };

        const handleMouseUp = () => {
            if (!isDragging.current) return;
            isDragging.current = false;
            
            // 移除性能优化类
            document.body.classList.remove('layout-sash-dragging');
            document.body.classList.remove('col-resize', 'row-resize');

            if (sashRef.current) {
                sashRef.current.classList.remove('active');
            }

            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };

        sash.addEventListener('mousedown', handleMouseDown);

        return () => {
            sash.removeEventListener('mousedown', handleMouseDown);
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };
    }, [orientation, beforeRef, afterRef, beforeNodeId, afterNodeId]);

    return (
        <div
            ref={sashRef}
            className={`
                relative z-30 transition-colors duration-150 group
                ${orientation === 'row' 
                    ? 'w-2 h-full cursor-col-resize -mx-1' 
                    : 'h-2 w-full cursor-row-resize -my-1'}
                hover:bg-blue-500/10 [&.active]:bg-blue-500/20
            `}
        >
            {/* Visual Line: Thin (1px or 4px) - 默认显示灰色便于区分，hover/active 时高亮为蓝色 */}
            <div 
                className={`
                    absolute bg-slate-400/25 group-hover:bg-blue-500 group-[.active]:bg-blue-500 transition-colors
                    ${orientation === 'row' 
                        ? 'left-1/2 -translate-x-1/2 w-[1px] h-full' 
                        : 'top-1/2 -translate-y-1/2 h-[1px] w-full'}
                `}
            />
        </div>
    );
};
