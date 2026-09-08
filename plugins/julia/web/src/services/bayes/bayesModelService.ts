import { invokeCommand } from "@/services/ipc";
import { parseValidationReportDTO } from "@/shared/types/bayes/wireParser";
import type {
  ParseExpressionRequestDTO,
  ParseExpressionResponseDTO,
  ValidationReportDTO,
  BayesModelDraftDTO,
} from "@/shared/types/bayes";

export async function parseBayesExpression(
  input: ParseExpressionRequestDTO,
): Promise<ParseExpressionResponseDTO> {
  return invokeCommand<ParseExpressionResponseDTO>("parse_bayes_expression", { input });
}

export async function validateBayesModel(input: BayesModelDraftDTO): Promise<ValidationReportDTO> {
  return parseValidationReportDTO(await invokeCommand<unknown>("validate_bayes_model", { input }));
}
