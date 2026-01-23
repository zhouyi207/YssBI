
import React from 'react';

export interface DropIndicatorProps {
    // We can pass the rect and the edge
    position: { top: number; left: number; width: number; height: number };
    visible: boolean;
    type?: 'dock' | 'merge'; // 'dock' is usually edge, 'merge' is center
}

export const DropIndicator: React.FC<DropIndicatorProps> = ({ position, visible, type }) => {
    if (!visible) return null;

    return (
        <div
            className="fixed pointer-events-none z-50 bg-[var(--accent-color)]/20 border-2 border-[var(--accent-color)] transition-all duration-75 ease-out"
            style={{
                ...position,
                opacity: visible ? 1 : 0,
                backgroundColor: type === 'merge' ? 'rgba(59, 130, 246, 0.4)' : 'transparent'
            }}
        >
            {/* Optional: Add icon or overlay for 'merge' vs 'dock' */}
        </div>
    );
};
