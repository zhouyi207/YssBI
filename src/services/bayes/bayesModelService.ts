import { invoke } from '@tauri-apps/api/core';
import type {
  ParseExpressionRequestDTO,
  ParseExpressionResponseDTO,
  ValidationReportDTO,
  BayesModelDraftDTO,
} from '@/shared/types/bayes';

export async function parseBayesExpression(input: ParseExpressionRequestDTO): Promise<ParseExpressionResponseDTO> {
  return invoke<ParseExpressionResponseDTO>('parse_bayes_expression', { input });
}

export async function validateBayesModel(input: BayesModelDraftDTO): Promise<ValidationReportDTO> {
  return invoke<ValidationReportDTO>('validate_bayes_model', { input });
}
