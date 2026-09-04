/**
 * DTO (Data Transfer Objects)
 *
 * IPC wire contracts and their boundary parsers.
 */

export * from "./database";
export * from "./project";
export * from "./applicationSettings";
export type * from "./editorProjection";
export * from "./editorMutation";
export type * from "./graphProjectionChannel";
export type * from "./clipboardSubgraph";
export * from "./runEvent";
export * from "./executionDemand";
export type * from "./diagnostics";
export type { IpcErrorDto } from "./ipcError";

export * from "./dataType";
export * from "./dataValue";
export * from "./variable";
