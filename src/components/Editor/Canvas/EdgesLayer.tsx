import { useRef, useCallback, useEffect, useLayoutEffect, useState } from "react";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore } from "../Store/useNodeStore";
import { useGestureStore } from "../Store/useGestureStore";
import { useExecutionStore } from "../Store/useExecutionStore";
import { drawEdge } from "../Edges/Edge";
import { DEFAULT_VIEWPORT } from "./constants";

// 🆕 粒子类型定义
interface Particle {
    connectionKey: string; // "fromPinId->toPinId"
    progress: number; // 0-1
    speed: number; // 每帧移动的距离
    size: number; // 粒子大小
    color: string; // 粒子颜色
}

export const EdgesLayer = ({
    groupId,
    visibleNodeIds,
    pinNodeIdIndex,
    getPinWorldPos,
    getCanvasLocalPoint,
    gesture,
    pendingConnection,
    contextMenu,
    activeTabId,
    theme
}: any) => {
    const edgeCanvasRef = useRef<HTMLCanvasElement>(null);
    const rafRef = useRef<number | null>(null);
    const isAnimatingRef = useRef(false);
    
    // 🆕 粒子系统
    const particlesRef = useRef<Particle[]>([]);
    const lastTimeRef = useRef<number>(0);
    
    // 🆕 是否启用数据流动画（可以通过设置控制）
    const [enableDataFlow, setEnableDataFlow] = useState(true);
    
    // 🆕 获取已完成的连接（显示数据流动画）
    const completedConnections = useExecutionStore((state) => state.completedConnections);
    // 获取活跃的连接（用于高亮显示）
    const activeConnections = useExecutionStore((state) => state.activeConnections);

    // 🆕 绘制粒子
    const drawParticle = useCallback((
        ctx: CanvasRenderingContext2D,
        x: number,
        y: number,
        size: number,
        color: string
    ) => {
        ctx.save();
        ctx.fillStyle = color;
        ctx.shadowColor = color;
        ctx.shadowBlur = size * 2;
        ctx.beginPath();
        ctx.arc(x, y, size, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();
    }, []);

    // 🆕 计算贝塞尔曲线上的点
    const getPointOnBezier = useCallback((
        t: number,
        x1: number,
        y1: number,
        x2: number,
        y2: number
    ) => {
        const dx = Math.abs(x1 - x2);
        const curvature = Math.max(dx * 0.5, 40);
        
        const c1x = x1 + curvature;
        const c1y = y1;
        const c2x = x2 - curvature;
        const c2y = y2;
        
        // 三次贝塞尔曲线公式
        const mt = 1 - t;
        const mt2 = mt * mt;
        const mt3 = mt2 * mt;
        const t2 = t * t;
        const t3 = t2 * t;
        
        const x = mt3 * x1 + 3 * mt2 * t * c1x + 3 * mt * t2 * c2x + t3 * x2;
        const y = mt3 * y1 + 3 * mt2 * t * c1y + 3 * mt * t2 * c2y + t3 * y2;
        
        return { x, y };
    }, []);

    // 🆕 更新粒子
    const updateParticles = useCallback((deltaTime: number) => {
        const particles = particlesRef.current;
        
        // 更新现有粒子
        for (let i = particles.length - 1; i >= 0; i--) {
            const particle = particles[i];
            particle.progress += particle.speed * deltaTime;
            
            // 移除完成的粒子
            if (particle.progress >= 1) {
                particles.splice(i, 1);
            }
        }
        
        // 为已完成的连接添加新粒子
        if (enableDataFlow) {
            completedConnections.forEach((connectionKey) => {
                // 检查是否已有粒子在这条连接上
                const existingCount = particles.filter(p => p.connectionKey === connectionKey).length;
                
                // 限制每条连接上的粒子数量
                if (existingCount < 3) {
                    // 随机决定是否添加新粒子（控制密度）
                    if (Math.random() < 0.1) {
                        particles.push({
                            connectionKey,
                            progress: 0,
                            speed: 0.01 + Math.random() * 0.01, // 0.01-0.02 per frame
                            size: 3 + Math.random() * 2, // 3-5px
                            color: '#10b981', // 绿色表示已完成
                        });
                    }
                }
            });
        }
    }, [completedConnections, enableDataFlow]);

    // 绘制连接线的核心逻辑 (GPU 加速)
    const drawAllEdges = useCallback((currentTime: number = 0) => {
        const canvasEl = edgeCanvasRef.current;
        if (!canvasEl) return;
        const ctx = canvasEl.getContext("2d");
        if (!ctx) return;

        const canvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;

        // 计算 deltaTime
        const deltaTime = lastTimeRef.current ? (currentTime - lastTimeRef.current) / 16.67 : 1;
        lastTimeRef.current = currentTime;

        // 🆕 更新粒子
        updateParticles(deltaTime);

        // 清除画布
        ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

        // 设置变换矩阵 (同步画布的平移和缩放)
        ctx.save();
        ctx.translate(canvas.x, canvas.y);
        ctx.scale(canvas.scale, canvas.scale);

        // 绘制已有连接
        const nodes = useNodeStore.getState().getNodes(activeTabId || "");

        nodes.forEach(node => {
            if (!node) return;

            node.outputs.forEach((pin: any) => {
                pin.links.forEach((targetId: string) => {
                    const start = getPinWorldPos(pin.id);
                    const end = getPinWorldPos(targetId);
                    if (!start || !end) return;

                    const connectionKey = `${pin.id}->${targetId}`;
                    const isActive = activeConnections.has(connectionKey);
                    const isCompleted = completedConnections.has(connectionKey);

                    // 绘制连接线
                    drawEdge(
                        ctx,
                        start.x, start.y,
                        end.x, end.y,
                        isActive ? '#facc15' : (pin.ui?.color ?? (theme[`${pin.type}Color` as keyof typeof theme] as string) ?? theme.connectionLines),
                        isActive ? 3 / canvas.scale : 2 / canvas.scale
                    );

                    // 🆕 绘制该连接上的粒子（仅在已完成的连接上）
                    if (isCompleted) {
                        const particles = particlesRef.current.filter(p => p.connectionKey === connectionKey);
                        particles.forEach(particle => {
                            const point = getPointOnBezier(
                                particle.progress,
                                start.x, start.y,
                                end.x, end.y
                            );
                            drawParticle(
                                ctx,
                                point.x,
                                point.y,
                                particle.size / canvas.scale,
                                particle.color
                            );
                        });
                    }
                });
            });
        });

        ctx.restore();
    }, [getPinWorldPos, theme, groupId, activeTabId, activeConnections, updateParticles, getPointOnBezier, drawParticle]);

    // 🆕 持续的动画循环（始终运行以支持粒子动画）
    const animate = useCallback((currentTime: number) => {
        drawAllEdges(currentTime);
        if (isAnimatingRef.current) {
            rafRef.current = requestAnimationFrame(animate);
        }
    }, [drawAllEdges]);

    // 🆕 启动动画循环
    const startAnimation = useCallback(() => {
        if (!isAnimatingRef.current) {
            isAnimatingRef.current = true;
            lastTimeRef.current = 0;
            rafRef.current = requestAnimationFrame(animate);
        }
    }, [animate]);

    // 🆕 停止动画循环
    const stopAnimation = useCallback(() => {
        isAnimatingRef.current = false;
        if (rafRef.current !== null) {
            cancelAnimationFrame(rafRef.current);
            rafRef.current = null;
        }
    }, []);

    // 🆕 始终运行动画循环（支持粒子动画）
    useEffect(() => {
        startAnimation();
        return () => {
            stopAnimation();
        };
    }, [startAnimation, stopAnimation]);

    // 监听手势状态（保留原有逻辑）
    useEffect(() => {
        const unsubGesture = useGestureStore.subscribe((state) => {
            const currentGesture = state.gesture;
            if (currentGesture && (currentGesture.type === "drag" || currentGesture.type === "pan")) {
                if (!isAnimatingRef.current) {
                    startAnimation();
                }
            }
        });

        return () => {
            unsubGesture();
        };
    }, [startAnimation]);

    // 监听 ViewportStore 和 NodeStore 的变化
    useEffect(() => {
        const unsubViewport = useViewportStore.subscribe(() => {
            if (!isAnimatingRef.current) {
                startAnimation();
            }
        });
        
        const unsubNodes = useNodeStore.subscribe(() => {
            if (!isAnimatingRef.current) {
                startAnimation();
            }
        });
        
        return () => {
            unsubViewport();
            unsubNodes();
        };
    }, [startAnimation]);

    // 同步画布尺寸并触发重绘
    useLayoutEffect(() => {
        const canvasEl = edgeCanvasRef.current;
        if (!canvasEl) return;

        // 确保从正确的父元素获取尺寸
        const rect = canvasEl.parentElement?.getBoundingClientRect();
        if (!rect || rect.width === 0 || rect.height === 0) return;

        const dpr = window.devicePixelRatio || 1;

        // 设置实际像素大小 (防止模糊)
        canvasEl.width = rect.width * dpr;
        canvasEl.height = rect.height * dpr;
        // 设置 CSS 大小
        canvasEl.style.width = `${rect.width}px`;
        canvasEl.style.height = `${rect.height}px`;

        const ctx = canvasEl.getContext("2d");
        if (ctx) {
            ctx.setTransform(dpr, 0, 0, dpr, 0, 0); // 使用 setTransform 替代 scale 避免累加
        }

        drawAllEdges();
    }, [drawAllEdges]);

    return <canvas ref={edgeCanvasRef} className="absolute inset-0 pointer-events-none" />;
};
