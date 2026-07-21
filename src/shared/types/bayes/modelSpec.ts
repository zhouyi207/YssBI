import type { ExpressionDTO, RawExpressionDTO } from './expression';
import type { InferenceConfigDTO } from './inferenceConfig';
import type { LikelihoodSpecDTO } from './likelihood';
import type { ParameterSpecDTO } from './prior';

export type BayesColumnDTypeDTO = 'number' | 'integer' | 'boolean' | 'string' | 'date' | 'unknown';

export interface BayesColumnMetaDTO {
  name: string;
  dtype: BayesColumnDTypeDTO;
  nullable: boolean;
}

export interface BayesDatasetSelectionDTO {
  sourceType: 'table' | 'query' | 'result_source';
  sourceId: string;
  columns: BayesColumnMetaDTO[];
}

export type BayesSymbolRoleDTO = 'dependent' | 'independent' | 'parameter';

export interface SymbolDraftDTO {
  name: string;
  role: BayesSymbolRoleDTO;
  inferredRole: BayesSymbolRoleDTO;
  userEdited: boolean;
}

export interface ResponseBindingDTO {
  symbol: string;
  column: string;
}

export interface FormulaDraftDTO {
  formulaText: string;
  rawResponse: RawExpressionDTO;
  rawPredictor: RawExpressionDTO;
}

export interface BayesModelDraftDTO {
  formulaText: string;
  rawResponse: RawExpressionDTO;
  rawPredictor: RawExpressionDTO | null;
  symbols: SymbolDraftDTO[];

  dataset: BayesDatasetSelectionDTO | null;
  responseBinding: ResponseBindingDTO | null;
  dataBindings: Record<string, string>;

  boundResponse: ExpressionDTO | null;
  boundPredictor: ExpressionDTO | null;
  likelihood: LikelihoodSpecDTO;
  parameters: ParameterSpecDTO[];
  sampler: InferenceConfigDTO;
}



export interface ParseExpressionRequestDTO {
  formula: string;
  columns?: BayesColumnMetaDTO[];
  symbols?: string[];
}

export interface ParseExpressionResponseDTO {
  formula: FormulaDraftDTO;
  symbols: string[];
}
