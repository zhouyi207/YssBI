import type { DetailTarget } from '@/features/core/editor/detail/types';
import type { GraphResourceRecord } from '@/features/core/resource/resourceSelectors';
import type { FunctionPinSpec } from '@/shared/types/domain/graph';
import type { Variable } from '@/shared/types/domain/variable';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type { DatabaseRecord } from '@/shared/types/dto/database';
import type { LogMessage } from '@/shared/types/ui';

export interface DetailCatalogSnapshot {
  variables: Record<string, Variable>;
  events: GraphResourceRecord;
  functions: GraphResourceRecord;
  dataframes: Record<string, DatabaseRecord>;
}

export interface DetailPanelResolveInput extends DetailCatalogSnapshot {
  target: DetailTarget | null;
  selectedLog: LogMessage | null;
  worksheetDocument: WorksheetDocument | null;
  functionSignature?: {
    functionInputs?: FunctionPinSpec[];
    functionOutputs?: FunctionPinSpec[];
  };
}

export type FunctionDetailModel = {
  id: string;
  name: string;
  inputs: FunctionPinSpec[];
  outputs: FunctionPinSpec[];
};

export type DetailPanelModel =
  | { kind: 'empty' }
  | { kind: 'log'; log: LogMessage }
  | { kind: 'node'; nodeId: string }
  | { kind: 'nodeDefinition'; nodeType: string }
  | { kind: 'variable'; id: string; variable: Variable }
  | { kind: 'event'; id: string; event: { id: string; name: string } }
  | { kind: 'function'; id: string; fn: FunctionDetailModel }
  | { kind: 'worksheet'; document: WorksheetDocument }
  | { kind: 'data'; id: string; dataframe: DatabaseRecord };

/** target + 目录快照 → Detail 面板判别联合（无回调，纯数据） */
export function resolveDetailPanelModel(input: DetailPanelResolveInput): DetailPanelModel {
  const {
    target,
    selectedLog,
    variables,
    events,
    functions,
    dataframes,
    worksheetDocument,
    functionSignature,
  } = input;

  if (!target) return { kind: 'empty' };

  switch (target.kind) {
    case 'log':
      return selectedLog ? { kind: 'log', log: selectedLog } : { kind: 'empty' };
    case 'node':
      return { kind: 'node', nodeId: target.id };
    case 'nodeDefinition':
      return { kind: 'nodeDefinition', nodeType: target.nodeType };
    case 'variable': {
      const variable = variables[target.id];
      return variable ? { kind: 'variable', id: target.id, variable } : { kind: 'empty' };
    }
    case 'event': {
      const event = events[target.id];
      return event ? { kind: 'event', id: target.id, event } : { kind: 'empty' };
    }
    case 'function': {
      const fnRecord = functions[target.id];
      if (!fnRecord) return { kind: 'empty' };
      return {
        kind: 'function',
        id: target.id,
        fn: {
          id: fnRecord.id,
          name: fnRecord.name,
          inputs: functionSignature?.functionInputs ?? [],
          outputs: functionSignature?.functionOutputs ?? [],
        },
      };
    }
    case 'worksheet':
      return worksheetDocument ? { kind: 'worksheet', document: worksheetDocument } : { kind: 'empty' };
    case 'data': {
      const dataframe = dataframes[target.id];
      return dataframe ? { kind: 'data', id: target.id, dataframe } : { kind: 'empty' };
    }
    default: {
      const exhaustive: never = target;
      return exhaustive;
    }
  }
}
