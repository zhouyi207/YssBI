export type GraphId = string;

export type GraphKind = 'event' | 'function' | 'macro';

export interface GraphPosition {
    x: number;
    y: number;
}

export interface Graph {
    id: GraphId;
    name: string;
    kind: GraphKind;
    position: GraphPosition;
}
