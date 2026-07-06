import React, { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { LayoutTab } from "@/shared/types";
import type { Graph } from "@/shared/types/domain";
import { useEditorGroup } from "@/features/application/editor";
import { useExecutionPlayback, useExecutionStore } from "@/features/core/execution";
import { UnifiedSourceView } from "@/features/core/resultSource";

import { useCanvasOverlayHandlers, type VariableDropMenu } from "@/features/application/editor";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ContextMenu } from "@/shared/ui/contextMenu";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { NodePalette, type PaletteItem } from "../../Layout/NodePalette";
import {
  VscDebugStart,
  VscDebugPause,
  VscDebugStop,
  VscDebugRestart,
  VscPlay,
  VscRunAll,
} from "react-icons/vsc";
import type { PinResultState } from "@/shared/types/ui";

const EMPTY_PIN_RESULTS = new Map<string, PinResultState>();

function CanvasToolbarButton({
    tooltip,
    children,
    ...props
}: React.ComponentProps<typeof Button> & { tooltip: string }) {
    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <Button {...props}>{children}</Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{tooltip}</TooltipContent>
        </Tooltip>
    );
}

export default function CanvasOverlays({
    canvasElementRef,
    variableDropMenu,
    setVariableDropMenu,
    onVariableDropGet,
    onVariableDropSet,
}: {
    canvasElementRef: React.RefObject<HTMLDivElement | null>;
    variableDropMenu: VariableDropMenu | null;
    setVariableDropMenu: (val: VariableDropMenu | null) => void;
    onVariableDropGet: (menu: VariableDropMenu) => void | Promise<void>;
    onVariableDropSet: (menu: VariableDropMenu) => void | Promise<void>;
}) {
    const { t } = useTranslation();
    const {
        contextMenu,
        setContextMenu,
        setPendingConnection,
        pendingConnection,
        variables,
        functions,
        tabs,
        activeTabId,
        activeGroupId,
        groupId,
        executeGraph,
        setCanvas,
        createNode,
    } = useEditorGroup();

    const {
        handleNodePaletteSelect,
    } = useCanvasOverlayHandlers({
        canvasElementRef,
        activeTabId,
        functions,
        pendingConnection,
        setContextMenu,
        setPendingConnection,
        createNode,
        setCanvas,
    });

    const tabId = activeTabId ?? "";
    const { stop, togglePlayPause, isPlaying, isPaused, hasRecording, graphDirty } = useExecutionPlayback(tabId);
    const graphStatus = useExecutionStore((s) => s.graphs[tabId]?.status ?? "idle");
    const pinResults = useExecutionStore((s) => s.graphs[tabId]?.pinResults ?? EMPTY_PIN_RESULTS);
    const pinResultList = useMemo(() => Array.from(pinResults.values()), [pinResults]);
    const [debugOpen, setDebugOpen] = useState(false);
    const [selectedSourceId, setSelectedSourceId] = useState<string | null>(null);
    const selectedResult = pinResultList.find((result) => result.sourceId === selectedSourceId) ?? pinResultList[0];

    const playbackActive = isPlaying || isPaused;
    const isThisGraphRunning = graphStatus === "running";
    const canReplay = hasRecording && !graphDirty && !isThisGraphRunning;

    const onPaletteSelect = (item: PaletteItem) =>
        contextMenu && handleNodePaletteSelect(item, contextMenu);

    const isEventTab = tabs.find((t: LayoutTab) => t.id === activeTabId)?.type === "event";

    return (
        <>
            {isEventTab && (
                <div className="absolute top-3 right-3 z-40 flex items-center gap-1 bg-[var(--panel-bg)]/80 backdrop-blur-sm border border-[var(--border-color)] rounded-md p-0.5 shadow-lg">
                    {/* Debug placeholder */}
                    <CanvasToolbarButton
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled={pinResultList.length === 0}
                        onClick={() => setDebugOpen((open) => !open)}
                        className={pinResultList.length > 0 ? 'text-blue-400 hover:text-blue-300' : 'text-[var(--text-secondary)] opacity-40'}
                        tooltip={pinResultList.length > 0 ? "Inspect runtime results" : t("canvas.debugComingSoon")}
                    >
                        <VscDebugStart size={14} />
                    </CanvasToolbarButton>

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    {!playbackActive ? (
                        <CanvasToolbarButton
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => canReplay && togglePlayPause()}
                            disabled={!canReplay}
                            className={
                                canReplay
                                    ? 'text-blue-400 hover:text-blue-300'
                                    : 'text-[var(--text-secondary)] opacity-40 cursor-not-allowed'
                            }
                            tooltip={
                                graphDirty ? t("canvas.replayDisabledDirty") :
                                !hasRecording ? t("canvas.replayNoRecording") :
                                t("canvas.replayExecution")
                            }
                        >
                            <VscDebugRestart size={14} />
                        </CanvasToolbarButton>
                    ) : (
                        <div className="flex items-center">
                            <CanvasToolbarButton
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={togglePlayPause}
                                className={isPlaying ? 'text-amber-400' : 'text-blue-400'}
                                tooltip={isPlaying ? t("canvas.pauseReplay") : t("canvas.resumeReplay")}
                            >
                                {isPlaying ? <VscDebugPause size={14} /> : <VscPlay size={14} />}
                            </CanvasToolbarButton>
                            <CanvasToolbarButton
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={stop}
                                className="text-red-400"
                                tooltip={t("canvas.stopReplay")}
                            >
                                <VscDebugStop size={14} />
                            </CanvasToolbarButton>
                        </div>
                    )}

                    <div className="w-px h-5 bg-[var(--border-color)]" />

                    <CanvasToolbarButton
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => !isThisGraphRunning && executeGraph(tabId)}
                        disabled={isThisGraphRunning}
                        className={
                            isThisGraphRunning
                                ? 'text-green-400 opacity-60 cursor-not-allowed'
                                : 'text-green-400 hover:text-green-300'
                        }
                        tooltip={isThisGraphRunning ? t("canvas.executing") : t("canvas.executeCurrentEvent")}
                    >
                        <VscRunAll size={14} />
                    </CanvasToolbarButton>
                </div>
            )}

            {debugOpen && pinResultList.length > 0 && (
                <div className="absolute right-3 top-14 z-40 flex max-h-[70vh] w-[520px] flex-col overflow-hidden rounded-md border border-[var(--border-color)] bg-[var(--panel-bg)] shadow-xl">
                    <div className="flex items-center justify-between border-b border-[var(--border-color)] px-3 py-2">
                        <span className="text-xs font-semibold text-foreground">Runtime Results</span>
                        <button
                            type="button"
                            className="text-xs text-muted-foreground hover:text-foreground"
                            onClick={() => setDebugOpen(false)}
                        >
                            Close
                        </button>
                    </div>
                    <div className="flex min-h-0 flex-1">
                        <div className="flex min-h-0 w-40 shrink-0 flex-col border-r border-[var(--border-color)]">
                          <OverlayScrollbar>
                            <div className="p-2">
                            {pinResultList.map((result) => (
                                <button
                                    key={result.sourceId}
                                    type="button"
                                    className={`block w-full truncate rounded px-2 py-1 text-left text-xs ${
                                        selectedResult?.sourceId === result.sourceId
                                            ? 'bg-[var(--accent-color)]/15 text-foreground'
                                            : 'text-muted-foreground hover:bg-muted'
                                    }`}
                                    onClick={() => setSelectedSourceId(result.sourceId)}
                                >
                                    {result.descriptor.title}
                                </button>
                            ))}
                            </div>
                          </OverlayScrollbar>
                        </div>
                        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                          <OverlayScrollbar>
                            <div className="p-2">
                              {selectedResult ? (
                                <UnifiedSourceView payload={selectedResult.descriptor} layout="embedded" />
                              ) : null}
                            </div>
                          </OverlayScrollbar>
                        </div>
                    </div>
                </div>
            )}

            {/* ================= Node Palette ================= */}
            {activeGroupId === groupId && contextMenu?.visible && createPortal(
                <div className="menu-container">
                    <NodePalette
                        x={contextMenu.x}
                        y={contextMenu.y}
                        onSelect={onPaletteSelect}
                        filterPin={pendingConnection}
                        variables={variables}
                        functions={functions as unknown as Record<string, Graph>}
                    />
                </div>,
                document.body
            )}

            {/* ================= Variable Drop Menu ================= */}
            {activeGroupId === groupId && variableDropMenu && (
                <ContextMenu
                    position={{ x: variableDropMenu.x, y: variableDropMenu.y }}
                    sections={[
                        {
                            items: [
                                {
                                    id: "get-variable",
                                    label: t("canvas.getVariable", { name: variableDropMenu.variableName }),
                                    onClick: () => void onVariableDropGet(variableDropMenu),
                                },
                            ],
                        },
                        {
                            items: [
                                {
                                    id: "set-variable",
                                    label: t("canvas.setVariable", { name: variableDropMenu.variableName }),
                                    onClick: () => void onVariableDropSet(variableDropMenu),
                                },
                            ],
                        },
                    ]}
                    onClose={() => setVariableDropMenu(null)}
                />
            )}
        </>
    );
}
