/** Selected serial-test result data, validated at the result-analysis boundary. */

export interface SerialTestWithLagDTO {
  stat: number;
  p_value: number;
  lags: number;
}

/** Durbin-Watson 结果（`{ d: number }`，非裸 number） */
export interface DurbinWatsonResultDTO {
  d: number;
}

export interface SerialTestsResponseDTO {
  bg?: SerialTestWithLagDTO;
  q?: SerialTestWithLagDTO;
  dw: DurbinWatsonResultDTO;
}
