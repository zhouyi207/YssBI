/** Coefficient rows shared by native linear-report queries and presentation components. */

export interface Coefficient {
  variable: string;
  coef: number;
  std_err?: number;
  t_value?: number;
  p_value?: number;
  "confidence_interval_0.025"?: number;
  "confidence_interval_0.975"?: number;
  is_significant: boolean;
}
