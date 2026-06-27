
import React, { useRef, useEffect } from 'react';
import { LayoutDirection } from '@/shared/types/ui';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';


interface SashProps {
    orientation: LayoutDirection;

    index: number;

    beforeRef: React.RefObject<HTMLDivElement | null>;
    afterRef: React.RefObject<HTMLDivElement | null>;

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
    const pendingResize = useRef<{ nodeId: string; size: number } | null>(null);
    const rafId = useRef<number | null>(null);
    const latestDelta = useRef(0);
    const cleanupDragListeners = useRef<(() => void) | null>(null);

    useEffect(() => {
        const sash = sashRef.current;
        if (!sash) return;

        const applyResize = (delta: number) => {
            if (!startSizes.current) return;

            const beforeEl = beforeRef.current;
            const afterEl = afterRef.current;
            if (!beforeEl || !afterEl) return;

            const { nodes } = useLayoutStore.getState();
            const beforeNode = nodes[beforeNodeId];
            const afterNode = nodes[afterNodeId];

            if (beforeNode?.pixelSize !== undefined) {
                const newSize = Math.max(beforeNode.minSize ?? 0, startSizes.current.before + delta);
                beforeEl.style.flex = `0 0 ${newSize}px`;
                pendingResize.current = { nodeId: beforeNodeId, size: newSize };
            } else if (afterNode?.pixelSize !== undefined) {
                const newSize = Math.max(afterNode.minSize ?? 0, startSizes.current.after - delta);
                afterEl.style.flex = `0 0 ${newSize}px`;
                pendingResize.current = { nodeId: afterNodeId, size: newSize };
            } else {
                const newSize = Math.max(beforeNode?.minSize ?? 0, startSizes.current.before + delta);
                beforeEl.style.flex = `0 0 ${newSize}px`;
                pendingResize.current = { nodeId: beforeNodeId, size: newSize };
            }
        };

        const restorePanelVisibility = () => {
            const { nodes, updateNode } = useLayoutStore.getState();
            const beforeNode = nodes[beforeNodeId];
            const afterNode = nodes[afterNodeId];

            if (beforeNode?.data?.visible === false) {
                const restored = { ...beforeNode.data, visible: true };
                if (!restored.currentTab && restored.component === 'Sidebar') restored.currentTab = 'graphs';
                updateNode(beforeNodeId, { data: restored });
            }
            if (afterNode?.data?.visible === false) {
                const restoredAfter = { ...afterNode.data, visible: true };
                updateNode(afterNodeId, { data: restoredAfter });
            }
        };

        const handleMouseDown = (e: MouseEvent) => {
            e.preventDefault();
            e.stopPropagation();

            const beforeEl = beforeRef.current;
            const afterEl = afterRef.current;

            if (!beforeEl || !afterEl) return;

            isDragging.current = true;
            pendingResize.current = null;
            latestDelta.current = 0;

            document.body.classList.add('layout-sash-dragging');
            document.body.classList.add(orientation === 'row' ? 'col-resize' : 'row-resize');

            if (sashRef.current) {
                sashRef.current.classList.add('active');
            }

            startPos.current = orientation === 'row' ? e.clientX : e.clientY;

            const beforeRect = beforeEl.getBoundingClientRect();
            const afterRect = afterEl.getBoundingClientRect();

            startSizes.current = {
                before: orientation === 'row' ? beforeRect.width : beforeRect.height,
                after: orientation === 'row' ? afterRect.width : afterRect.height
            };

            restorePanelVisibility();

            cleanupDragListeners.current?.();
            const cleanupMouseMove = addGlobalEventListener(window, 'mousemove', handleMouseMove);
            const cleanupMouseUp = addGlobalEventListener(window, 'mouseup', handleMouseUp);
            cleanupDragListeners.current = () => {
                cleanupMouseMove();
                cleanupMouseUp();
            };
        };

        const handleMouseMove = (e: MouseEvent) => {
            if (!isDragging.current || !startSizes.current) return;

            latestDelta.current = (orientation === 'row' ? e.clientX : e.clientY) - startPos.current;

            if (rafId.current !== null) return;
            rafId.current = requestAnimationFrame(() => {
                rafId.current = null;
                applyResize(latestDelta.current);
            });
        };

        const handleMouseUp = () => {
            if (!isDragging.current) return;
            isDragging.current = false;

            if (rafId.current !== null) {
                cancelAnimationFrame(rafId.current);
                rafId.current = null;
                applyResize(latestDelta.current);
            }

            if (pendingResize.current) {
                useLayoutStore.getState().resizeNode(
                    pendingResize.current.nodeId,
                    pendingResize.current.size,
                );
                pendingResize.current = null;
            }

            document.body.classList.remove('layout-sash-dragging');
            document.body.classList.remove('col-resize', 'row-resize');
            window.dispatchEvent(new CustomEvent('layout-sash-drag-end'));

            if (sashRef.current) {
                sashRef.current.classList.remove('active');
            }

            cleanupDragListeners.current?.();
            cleanupDragListeners.current = null;
        };

        sash.addEventListener('mousedown', handleMouseDown);

        return () => {
            sash.removeEventListener('mousedown', handleMouseDown);
            cleanupDragListeners.current?.();
            cleanupDragListeners.current = null;
            if (rafId.current !== null) {
                cancelAnimationFrame(rafId.current);
            }
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
                hover:bg-primary/10 [&.active]:bg-primary/20
            `}
        >
            <div 
                className={`
                    absolute bg-border/60 group-hover:bg-primary group-[.active]:bg-primary transition-colors
                    ${orientation === 'row' 
                        ? 'left-1/2 -translate-x-1/2 w-[1px] h-full' 
                        : 'top-1/2 -translate-y-1/2 h-[1px] w-full'}
                `}
            />
        </div>
    );
};
