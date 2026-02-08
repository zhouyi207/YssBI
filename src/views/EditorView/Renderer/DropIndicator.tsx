
import React from 'react';

export interface DropIndicatorProps {
    // We can pass the rect and the edge
    position: { top: number; left: number; width: number; height: number };
    visible: boolean;
    type?: 'dock' | 'merge'; // 'dock' is usually edge, 'merge' is center
}

export const DropIndicator: React.FC<DropIndicatorProps> = ({ position, visible, type }) => {
    // 保持组件挂载以允许 CSS 过渡动画
    return (
        <div
            className="fixed pointer-events-none z-[100] bg-[var(--accent-color)]/30 border-2 border-[var(--accent-color)] transition-all duration-200 ease-out"
            style={{
                ...position,
                opacity: visible ? 1 : 0,
                // 当不可见时，也将尺寸设为 0 以防万一
                // width: visible ? position.width : 0,
                // height: visible ? position.height : 0,
                backgroundColor: type === 'merge' ? 'rgba(59, 130, 246, 0.4)' : 'rgba(var(--accent-color-rgb), 0.2)'
            }}
        >
            <div className="absolute inset-0 bg-white/10" />
        </div>
    );
};
