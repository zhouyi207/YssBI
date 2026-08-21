import type { BayesDatasetSelectionDTO } from '@/shared/types/bayes';

export interface BayesDatasetOption extends BayesDatasetSelectionDTO {
  displayName: string;
}

export type Translation = (key: string) => string;
