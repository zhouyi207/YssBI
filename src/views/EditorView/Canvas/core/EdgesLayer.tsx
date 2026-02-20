import { useRef, useCallback, useEffect, useLayoutEffect, useState, useMemo } from "react";
import { useViewportStore } from '@/features/core/viewport';
import { useGraphData, useGraphDataStore } from "@/features/core/dataStore";
import { useGestureStore } from '@/features/core/gesture';
import { useExecutionStore } from "@/features/core/execution";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { drawEdge } from "./Edge";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";
import { deserializeGraph } from "@/features/core/dataStore";

interface Particle {
    connectionKey: string;
    progress: number;
    speed: number;
    size: number;
    color: string;
}

export const EdgesLayer = ({
    groupId,
    getPinWorldPos,
    activeTabId,
}: {
    groupId: string;
    getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
    activeTabId: string | null;
}) => {
    const edgeCanvasRef = useRef<HTMLCanvasElement>(null);
    const rafRef = useRef<number | null>(null);
    const isAnimatingRef = useRef(false);
    const particlesRef = useRef<Particle[]>([]);
    const lastTimeRef = useRef<number>(0);

    const { theme } = useTheme();

    const completedConnections = useExecutionStore((state) => state.completedConnections);
    const activeConnections = useExecutionStore((state) => state.activeConnections);

    const graphData = useGraphData(activeTabId);
    const nodes = useMemo(() => {
        if (!graphData) return [];
        return deserializeGraph(graphData).nodes;
    }, [graphData]);

    // ============================================================
    // 将所有动画帧需要的数据存入 ref，使 drawAllEdges 保持稳定引用
    // ============================================================
    const getPinWorldPosRef = useRef(getPinWorldPos);
    const themeRef = useRef(theme);
    const nodesRef = useRef(nodes);
    const activeConnectionsRef = useRef(activeConnections);
    const completedConnectionsRef = useRef(completedConnections);
    const groupIdRef = useRef(groupId);

    useEffect(() => { getPinWorldPosRef.current = getPinWorldPos; }, [getPinWorldPos]);
    useEffect(() => { themeRef.current = theme; }, [theme]);
    useEffect(() => { nodesRef.current = nodes; }, [nodes]);
    useEffect(() => { activeConnectionsRef.current = activeConnections; }, [activeConnections]);
    useEffect(() => { completedConnectionsRef.current = completedConnections; }, [completedConnections]);
    useEffect(() => { groupIdRef.current = groupId; }, [groupId]);

    const getPointOnBezier = useCallback((
        t: number, x1: number, y1: number, x2: number, y2: number
    ) => {
        const dx = Math.abs(x1 - x2);
        const curvature = Math.max(dx * 0.5, 40);
        const c1x = x1 + curvature, c1y = y1;
        const c2x = x2 - curvature, c2y = y2;
        const mt = 1 - t, mt2 = mt * mt, mt3 = mt2 * mt;
        const t2 = t * t, t3 = t2 * t;
        return {
            x: mt3 * x1 + 3 * mt2 * t * c1x + 3 * mt * t2 * c2x + t3 * x2,
            y: mt3 * y1 + 3 * mt2 * t * c1y + 3 * mt * t2 * c2y + t3 * y2,
        };
    }, []);

    // ============================================================
    // drawAllEdges — 稳定引用，通过 ref 读取最新数据
    // ============================================================
    const drawAllEdges = useCallback((currentTime: number = 0) => {
        const canvasEl = edgeCanvasRef.current;
        if (!canvasEl) return;
        const ctx = canvasEl.getContext("2d");
        if (!ctx) return;

        const canvas = useViewportStore.getState().viewports[groupIdRef.current] || DEFAULT_VIEWPORT;
        const deltaTime = lastTimeRef.current ? (currentTime - lastTimeRef.current) / 16.67 : 1;
        lastTimeRef.current = currentTime;

        const particles = particlesRef.current;
        const completed = completedConnectionsRef.current;
        const active = activeConnectionsRef.current;

        // 更新粒子
        for (let i = particles.length - 1; i >= 0; i--) {
            particles[i].progress += particles[i].speed * deltaTime;
            if (particles[i].progress >= 1) particles.splice(i, 1);
        }

        if (completed.size > 0) {
            completed.forEach((connectionKey) => {
                const existingCount = particles.filter(p => p.connectionKey === connectionKey).length;
                if (existingCount < 3 && Math.random() < 0.1) {
                    particles.push({
                        connectionKey,
                        progress: 0,
                        speed: 0.01 + Math.random() * 0.01,
                        size: 3 + Math.random() * 2,
                        color: '#10b981',
                    });
                }
            });
        }

        ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);
        ctx.save();
        ctx.translate(canvas.x, canvas.y);
        ctx.scale(canvas.scale, canvas.scale);

        const currentTheme = themeRef.current;
        const currentGetPinWorldPos = getPinWorldPosRef.current;
        const currentNodes = nodesRef.current;

        for (const node of currentNodes) {
            if (!node) continue;
            for (const pin of node.outputs) {
                for (const targetId of pin.links) {
                    const start = currentGetPinWorldPos(pin.id);
                    const end = currentGetPinWorldPos(targetId);
                    if (!start || !end) continue;

                    const connectionKey = `${pin.id}->${targetId}`;
                    const isActive = active.has(connectionKey);
                    const isCompleted = completed.has(connectionKey);

                    drawEdge(
                        ctx,
                        start.x, start.y,
                        end.x, end.y,
                        isActive ? '#facc15' : (pin.ui?.color ?? getPinTypeColor(pin.type ?? "any", currentTheme)),
                        isActive ? 3 / canvas.scale : 2 / canvas.scale
                    );

                    if (isCompleted) {
                        for (const particle of particles.filter(p => p.connectionKey === connectionKey)) {
                            const point = getPointOnBezier(particle.progress, start.x, start.y, end.x, end.y);
                            ctx.save();
                            ctx.fillStyle = particle.color;
                            ctx.shadowColor = particle.color;
                            ctx.shadowBlur = particle.size * 2;
                            ctx.beginPath();
                            ctx.arc(point.x, point.y, particle.size / canvas.scale, 0, Math.PI * 2);
                            ctx.fill();
                            ctx.restore();
                        }
                    }
                }
            }
        }

        ctx.restore();
    }, [getPointOnBezier]); // 稳定依赖 — 不含 props/state

    // ============================================================
    // 动画循环：稳定引用，不会因 props 变化而重建
    // ============================================================
    const animate = useCallback((currentTime: number) => {
        drawAllEdges(currentTime);
        if (isAnimatingRef.current) {
            rafRef.current = requestAnimationFrame(animate);
        }
    }, [drawAllEdges]);

    const requestRedraw = useCallback(() => {
        if (!isAnimatingRef.current) {
            rafRef.current = requestAnimationFrame((t) => {
                drawAllEdges(t);
            });
        }
    }, [drawAllEdges]);

    const startAnimation = useCallback(() => {
        if (!isAnimatingRef.current) {
            isAnimatingRef.current = true;
            lastTimeRef.current = 0;
            rafRef.current = requestAnimationFrame(animate);
        }
    }, [animate]);

    const stopAnimation = useCallback(() => {
        isAnimatingRef.current = false;
        if (rafRef.current !== null) {
            cancelAnimationFrame(rafRef.current);
            rafRef.current = null;
        }
    }, []);

    // 仅在有粒子时运行持续动画，否则按需单帧重绘
    useEffect(() => {
        if (completedConnections.size > 0) {
            startAnimation();
        } else {
            stopAnimation();
            requestRedraw();
        }
        return () => stopAnimation();
    }, [completedConnections.size, startAnimation, stopAnimation, requestRedraw]);

    // 手势活跃时运行持续动画
    useEffect(() => {
        const unsubGesture = useGestureStore.subscribe((state) => {
            const g = state.gesture;
            if (g && (g.type === "drag" || g.type === "pan" || g.type === "connect")) {
                startAnimation();
            } else {
                // 手势结束，最后画一帧然后停下
                if (completedConnectionsRef.current.size === 0) {
                    stopAnimation();
                    requestRedraw();
                }
            }
        });
        return () => unsubGesture();
    }, [startAnimation, stopAnimation, requestRedraw]);

    // viewport/数据变化时触发单帧重绘
    useEffect(() => {
        const unsubViewport = useViewportStore.subscribe(() => requestRedraw());
        const unsubGraphData = useGraphDataStore.subscribe(() => requestRedraw());
        return () => { unsubViewport(); unsubGraphData(); };
    }, [requestRedraw]);

    // 同步画布尺寸
    useLayoutEffect(() => {
        const canvasEl = edgeCanvasRef.current;
        if (!canvasEl) return;
        const rect = canvasEl.parentElement?.getBoundingClientRect();
        if (!rect || rect.width === 0 || rect.height === 0) return;
        const dpr = window.devicePixelRatio || 1;
        canvasEl.width = rect.width * dpr;
        canvasEl.height = rect.height * dpr;
        canvasEl.style.width = `${rect.width}px`;
        canvasEl.style.height = `${rect.height}px`;
        const ctx = canvasEl.getContext("2d");
        if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        drawAllEdges();
    }, [drawAllEdges]);

    return <canvas ref={edgeCanvasRef} className="absolute inset-0 pointer-events-none" />;
};
