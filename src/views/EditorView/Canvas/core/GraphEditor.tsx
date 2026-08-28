import { memo } from 'react';
import Canvas from './Canvas';
import { useIsActiveEditorPanel } from '@/features/application/editor';
import { CanvasDropZone } from './CanvasDropZone';
import { useGraphDataStore, useProjectIOStore, resourceKey, useDocumentStateStore, type EditorResourceKind } from '@/features/application/viewCapabilities';

export interface GraphEditorProps {
    panelInstanceId: string;
    groupId: string;
    graphPath: string;
    graphKind: Exclude<EditorResourceKind, 'worksheet'>;
}

/**
 * Graph editor shell per Dockview panel.
 * Each panel renders its own resource; only the physically active panel is interactive.
 */
export const GraphEditor = memo(function GraphEditor({
    panelInstanceId,
    groupId,
    graphPath,
    graphKind,
}: GraphEditorProps) {
    const mode = useIsActiveEditorPanel(panelInstanceId) ? 'interactive' : 'preview';
    const graphLoadStatus = useProjectIOStore((state) => (
        state.graphLoadStatus[graphPath]
    ));
    const graphProjectionReady = useGraphDataStore((state) => (
        state.hasGraph(graphPath)
    ));
    const graphDocument = useDocumentStateStore((state) => (
        state.documents[resourceKey({ id: graphPath, kind: graphKind })]
    ));
    const graphReady = graphProjectionReady
        && graphDocument?.loaded === true
        && graphDocument.stale === false
        && graphDocument.conflict === false
        && graphLoadStatus !== 'loading'
        && graphLoadStatus !== 'error';
    const graphUnavailable = graphLoadStatus === 'error' || graphDocument?.conflict === true;

    return (
        <div className="flex flex-col w-full h-full min-h-0 min-w-0 overflow-hidden">
            <div className="flex-1 relative min-h-0 min-w-0 overflow-hidden">
                <CanvasDropZone
                    panelInstanceId={panelInstanceId}
                    groupId={groupId}
                    graphPath={graphPath}
                    graphKind={graphKind}
                    mode={mode}
                >
                    {graphReady
                        ? (
                            <Canvas
                                mode={mode}
                                panelInstanceId={panelInstanceId}
                                groupId={groupId}
                                graphPath={graphPath}
                                graphKind={graphKind}
                            />
                        )
                        : graphUnavailable
                            ? <div className="absolute inset-0" role="alert" data-graph-load-error />
                            : <div className="absolute inset-0" aria-busy="true" data-graph-loading />}
                </CanvasDropZone>
            </div>
        </div>
    );
});

export default GraphEditor;
