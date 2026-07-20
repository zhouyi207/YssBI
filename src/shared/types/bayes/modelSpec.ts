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
  column: string;
  symbol?: string;
}

export interface FormulaDraftDTO {
  formulaText: string;
  responseSymbol?: string;
  rawPredictor: RawExpressionDTO | null;
}

export interface BayesModelDraftDTO {
  formulaText: string;
  responseSymbol?: string;
  rawPredictor: RawExpressionDTO | null;
  symbols: SymbolDraftDTO[];

  dataset: BayesDatasetSelectionDTO | null;
  responseBinding: ResponseBindingDTO | null;
  dataBindings: Record<string, string>;

  boundPredictor: ExpressionDTO | null;
  likelihood: LikelihoodSpecDTO;
  parameters: ParameterSpecDTO[];
  sampler: InferenceConfigDTO;
}

export interface BayesModelSpecDTO {
  responseColumn: string;
  responseSymbol?: string;
  predictor: ExpressionDTO;
  dataVariables: Record<string, string>;
  likelihood: LikelihoodSpecDTO;
  parameters: ParameterSpecDTO[];
}

export interface ParseExpressionRequestDTO {
  formula: string;
  columns?: BayesColumnMetaDTO[];
}

export interface ParseExpressionResponseDTO {
  formula: FormulaDraftDTO;
  symbols: string[];
}
