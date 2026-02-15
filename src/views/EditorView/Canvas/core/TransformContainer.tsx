import { useRef, useEffect } from "react";
import { useViewportStore } from "@/features/core/viewport";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";

export const TransformContainer = ({ groupId, children }: { groupId: string, children: React.ReactNode }) => {
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        // 零 React 重绘：平移缩放时直接操作 transform，跳过 Virtual DOM Diff
        return useViewportStore.subscribe(state => {
            const canvas = state.viewports[groupId] || DEFAULT_VIEWPORT;
            const el = containerRef.current;
            if (el) {
                // 使用 translate3d 触发 GPU 加速，确保 CSS 格式正确
                el.style.transform = `translate3d(${canvas.x}px, ${canvas.y}px, 0) scale(${canvas.scale})`;
            }
        });
    }, [groupId]);

    const initial = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
    return (
        <div
            ref={containerRef}
            style={{
                transform: `translate3d(${initial.x}px, ${initial.y}px, 0) scale(${initial.scale})`,
                transformOrigin: "0 0",
                // 移除 will-change: transform，防止浏览器锁定低分辨率快照
                // 使用 backface-visibility 确保在某些浏览器中保持 3D 上下文但减少模糊
                backfaceVisibility: "hidden",
                WebkitBackfaceVisibility: "hidden",
            }}
        >
            {children}
        </div>
    );
};
