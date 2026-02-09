
import React, { useRef, useEffect } from 'react';
import { LayoutDirection } from '../../../shared/types/layout';
import { useLayoutStore } from '../../../features/layoutStore/layoutStore';


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

            const { nodes, resizeNode } = useLayoutStore.getState();
            const beforeNode = nodes[beforeNodeId];
            const afterNode = nodes[afterNodeId];

            // 实时更新 Store，触发 React 重新渲染
            if (beforeNode?.pixelSize !== undefined) {
                const newSize = Math.max(beforeNode.minSize ?? 0, startSizes.current.before + delta);
                resizeNode(beforeNodeId, newSize);
            } else if (afterNode?.pixelSize !== undefined) {
                const newSize = Math.max(afterNode.minSize ?? 0, startSizes.current.after - delta);
                resizeNode(afterNodeId, newSize);
            } else {
                // 如果两个节点都是 flex 模式（都没有 pixelSize），
                // 则将前一个节点转换为 pixelSize 模式，使其在调节时能够保持固定尺寸
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
                relative z-[100] transition-colors duration-150 group
                ${orientation === 'row' 
                    ? 'w-2 h-full cursor-col-resize -mx-1' 
                    : 'h-2 w-full cursor-row-resize -my-1'}
                hover:bg-blue-500/10 [&.active]:bg-blue-500/20
            `}
        >
            {/* Visual Line: Thin (1px or 4px) */}
            <div 
                className={`
                    absolute bg-transparent group-hover:bg-blue-500 group-[.active]:bg-blue-500 transition-colors
                    ${orientation === 'row' 
                        ? 'left-1/2 -translate-x-1/2 w-[1px] h-full' 
                        : 'top-1/2 -translate-y-1/2 h-[1px] w-full'}
                `}
            />
        </div>
    );
};
