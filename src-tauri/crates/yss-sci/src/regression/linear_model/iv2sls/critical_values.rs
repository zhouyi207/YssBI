use super::types::{StockYogoBiasRow, StockYogoCriticalValues, StockYogoSizeRow};

/// Stock-Yogo (2005) 临界值，1 内生变量。k2=排除工具数。与 Stata ivreg2/estat firststage 一致。
/// 来源: livreg2.do s_ivbias*, s_ivsize*
/// bias 在 k2=1,2 时为 None（Stock-Yogo 未提供）
pub(super) fn stock_yogo_cv_1_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let (bias, size) = match k2 {
        1 => (
            None,
            StockYogoSizeRow {
                pct_10: 16.38,
                pct_15: 8.96,
                pct_20: 6.66,
                pct_25: 5.53,
            },
        ),
        2 => (
            None,
            StockYogoSizeRow {
                pct_10: 19.93,
                pct_15: 11.59,
                pct_20: 8.75,
                pct_25: 7.25,
            },
        ),
        3 => (
            Some(StockYogoBiasRow {
                pct_5: 22.30,
                pct_10: 12.83,
                pct_20: 7.80,
                pct_30: 5.91,
            }),
            StockYogoSizeRow {
                pct_10: 22.30,
                pct_15: 12.83,
                pct_20: 9.54,
                pct_25: 7.80,
            },
        ),
        4 => (
            Some(StockYogoBiasRow {
                pct_5: 16.85,
                pct_10: 10.27,
                pct_20: 6.71,
                pct_30: 5.34,
            }),
            StockYogoSizeRow {
                pct_10: 24.58,
                pct_15: 13.96,
                pct_20: 10.26,
                pct_25: 8.31,
            },
        ),
        5 => (
            Some(StockYogoBiasRow {
                pct_5: 18.37,
                pct_10: 10.91,
                pct_20: 7.03,
                pct_30: 5.54,
            }),
            StockYogoSizeRow {
                pct_10: 26.87,
                pct_15: 15.09,
                pct_20: 10.98,
                pct_25: 8.84,
            },
        ),
        6 => (
            Some(StockYogoBiasRow {
                pct_5: 19.86,
                pct_10: 11.52,
                pct_20: 7.34,
                pct_30: 5.73,
            }),
            StockYogoSizeRow {
                pct_10: 29.18,
                pct_15: 16.23,
                pct_20: 11.72,
                pct_25: 9.38,
            },
        ),
        7 => (
            Some(StockYogoBiasRow {
                pct_5: 21.33,
                pct_10: 12.12,
                pct_20: 7.64,
                pct_30: 5.91,
            }),
            StockYogoSizeRow {
                pct_10: 31.50,
                pct_15: 17.38,
                pct_20: 12.48,
                pct_25: 9.93,
            },
        ),
        8 => (
            Some(StockYogoBiasRow {
                pct_5: 22.78,
                pct_10: 12.70,
                pct_20: 7.93,
                pct_30: 6.08,
            }),
            StockYogoSizeRow {
                pct_10: 33.84,
                pct_15: 18.54,
                pct_20: 13.24,
                pct_25: 10.50,
            },
        ),
        9 => (
            Some(StockYogoBiasRow {
                pct_5: 24.21,
                pct_10: 13.27,
                pct_20: 8.21,
                pct_30: 6.25,
            }),
            StockYogoSizeRow {
                pct_10: 36.19,
                pct_15: 19.71,
                pct_20: 14.01,
                pct_25: 11.07,
            },
        ),
        10 => (
            Some(StockYogoBiasRow {
                pct_5: 25.63,
                pct_10: 13.83,
                pct_20: 8.48,
                pct_30: 6.41,
            }),
            StockYogoSizeRow {
                pct_10: 38.54,
                pct_15: 20.88,
                pct_20: 14.78,
                pct_25: 11.65,
            },
        ),
        11 => (
            Some(StockYogoBiasRow {
                pct_5: 27.03,
                pct_10: 14.38,
                pct_20: 8.75,
                pct_30: 6.57,
            }),
            StockYogoSizeRow {
                pct_10: 40.90,
                pct_15: 22.06,
                pct_20: 15.56,
                pct_25: 12.23,
            },
        ),
        12 => (
            Some(StockYogoBiasRow {
                pct_5: 28.42,
                pct_10: 14.92,
                pct_20: 9.01,
                pct_30: 6.72,
            }),
            StockYogoSizeRow {
                pct_10: 43.27,
                pct_15: 23.24,
                pct_20: 16.35,
                pct_25: 12.82,
            },
        ),
        13 => (
            Some(StockYogoBiasRow {
                pct_5: 29.80,
                pct_10: 15.45,
                pct_20: 9.26,
                pct_30: 6.87,
            }),
            StockYogoSizeRow {
                pct_10: 45.64,
                pct_15: 24.42,
                pct_20: 17.14,
                pct_25: 13.41,
            },
        ),
        14 => (
            Some(StockYogoBiasRow {
                pct_5: 31.16,
                pct_10: 15.97,
                pct_20: 9.51,
                pct_30: 7.01,
            }),
            StockYogoSizeRow {
                pct_10: 48.01,
                pct_15: 25.61,
                pct_20: 17.93,
                pct_25: 14.00,
            },
        ),
        15 => (
            Some(StockYogoBiasRow {
                pct_5: 32.52,
                pct_10: 16.49,
                pct_20: 9.75,
                pct_30: 7.15,
            }),
            StockYogoSizeRow {
                pct_10: 50.39,
                pct_15: 26.80,
                pct_20: 18.72,
                pct_25: 14.60,
            },
        ),
        16 => (
            Some(StockYogoBiasRow {
                pct_5: 33.86,
                pct_10: 17.00,
                pct_20: 9.99,
                pct_30: 7.28,
            }),
            StockYogoSizeRow {
                pct_10: 52.77,
                pct_15: 27.99,
                pct_20: 19.51,
                pct_25: 15.19,
            },
        ),
        17 => (
            Some(StockYogoBiasRow {
                pct_5: 35.20,
                pct_10: 17.50,
                pct_20: 10.22,
                pct_30: 7.41,
            }),
            StockYogoSizeRow {
                pct_10: 55.15,
                pct_15: 29.19,
                pct_20: 20.31,
                pct_25: 15.79,
            },
        ),
        18 => (
            Some(StockYogoBiasRow {
                pct_5: 36.52,
                pct_10: 18.00,
                pct_20: 10.45,
                pct_30: 7.54,
            }),
            StockYogoSizeRow {
                pct_10: 57.53,
                pct_15: 30.38,
                pct_20: 21.10,
                pct_25: 16.39,
            },
        ),
        19 => (
            Some(StockYogoBiasRow {
                pct_5: 37.84,
                pct_10: 18.49,
                pct_20: 10.67,
                pct_30: 7.66,
            }),
            StockYogoSizeRow {
                pct_10: 59.92,
                pct_15: 31.58,
                pct_20: 21.90,
                pct_25: 16.99,
            },
        ),
        20 => (
            Some(StockYogoBiasRow {
                pct_5: 39.15,
                pct_10: 18.97,
                pct_20: 10.89,
                pct_30: 7.78,
            }),
            StockYogoSizeRow {
                pct_10: 62.30,
                pct_15: 32.77,
                pct_20: 22.70,
                pct_25: 17.60,
            },
        ),
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias, size })
}

/// Stock-Yogo (2005) 临界值，2 内生变量。k2=排除工具数。与 Stata ivreg2/estat firststage 一致。
/// 来源: livreg2.do s_ivbias*, s_ivsize* (K1=2 列)
/// bias 在 k2=2,3 时为 None（Stock-Yogo 未提供）
pub(super) fn stock_yogo_cv_2_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let (bias, size) = match k2 {
        2 => (
            None,
            StockYogoSizeRow {
                pct_10: 7.03,
                pct_15: 4.58,
                pct_20: 3.95,
                pct_25: 3.63,
            },
        ),
        3 => (
            None,
            StockYogoSizeRow {
                pct_10: 13.43,
                pct_15: 8.18,
                pct_20: 6.40,
                pct_25: 5.45,
            },
        ),
        4 => (
            Some(StockYogoBiasRow {
                pct_5: 11.04,
                pct_10: 7.56,
                pct_20: 5.57,
                pct_30: 4.73,
            }),
            StockYogoSizeRow {
                pct_10: 16.87,
                pct_15: 9.93,
                pct_20: 7.54,
                pct_25: 6.28,
            },
        ),
        5 => (
            Some(StockYogoBiasRow {
                pct_5: 12.16,
                pct_10: 8.18,
                pct_20: 5.91,
                pct_30: 4.96,
            }),
            StockYogoSizeRow {
                pct_10: 19.45,
                pct_15: 11.22,
                pct_20: 8.38,
                pct_25: 6.89,
            },
        ),
        6 => (
            Some(StockYogoBiasRow {
                pct_5: 13.27,
                pct_10: 8.79,
                pct_20: 6.23,
                pct_30: 5.18,
            }),
            StockYogoSizeRow {
                pct_10: 21.68,
                pct_15: 12.33,
                pct_20: 9.10,
                pct_25: 7.42,
            },
        ),
        7 => (
            Some(StockYogoBiasRow {
                pct_5: 14.36,
                pct_10: 9.39,
                pct_20: 6.54,
                pct_30: 5.39,
            }),
            StockYogoSizeRow {
                pct_10: 23.72,
                pct_15: 13.34,
                pct_20: 9.77,
                pct_25: 7.91,
            },
        ),
        8 => (
            Some(StockYogoBiasRow {
                pct_5: 15.45,
                pct_10: 9.98,
                pct_20: 6.84,
                pct_30: 5.59,
            }),
            StockYogoSizeRow {
                pct_10: 25.64,
                pct_15: 14.31,
                pct_20: 10.41,
                pct_25: 8.39,
            },
        ),
        9 => (
            Some(StockYogoBiasRow {
                pct_5: 16.53,
                pct_10: 10.56,
                pct_20: 7.13,
                pct_30: 5.78,
            }),
            StockYogoSizeRow {
                pct_10: 27.51,
                pct_15: 15.24,
                pct_20: 11.03,
                pct_25: 8.85,
            },
        ),
        10 => (
            Some(StockYogoBiasRow {
                pct_5: 17.60,
                pct_10: 11.13,
                pct_20: 7.41,
                pct_30: 5.97,
            }),
            StockYogoSizeRow {
                pct_10: 29.32,
                pct_15: 16.16,
                pct_20: 11.65,
                pct_25: 9.31,
            },
        ),
        11 => (
            Some(StockYogoBiasRow {
                pct_5: 18.66,
                pct_10: 11.70,
                pct_20: 7.69,
                pct_30: 6.15,
            }),
            StockYogoSizeRow {
                pct_10: 31.11,
                pct_15: 17.06,
                pct_20: 12.25,
                pct_25: 9.77,
            },
        ),
        12 => (
            Some(StockYogoBiasRow {
                pct_5: 19.72,
                pct_10: 12.26,
                pct_20: 7.96,
                pct_30: 6.33,
            }),
            StockYogoSizeRow {
                pct_10: 32.88,
                pct_15: 17.95,
                pct_20: 12.86,
                pct_25: 10.22,
            },
        ),
        13 => (
            Some(StockYogoBiasRow {
                pct_5: 20.77,
                pct_10: 12.81,
                pct_20: 8.23,
                pct_30: 6.50,
            }),
            StockYogoSizeRow {
                pct_10: 34.62,
                pct_15: 18.84,
                pct_20: 13.45,
                pct_25: 10.68,
            },
        ),
        14 => (
            Some(StockYogoBiasRow {
                pct_5: 21.81,
                pct_10: 13.36,
                pct_20: 8.49,
                pct_30: 6.67,
            }),
            StockYogoSizeRow {
                pct_10: 36.36,
                pct_15: 19.72,
                pct_20: 14.05,
                pct_25: 11.13,
            },
        ),
        15 => (
            Some(StockYogoBiasRow {
                pct_5: 22.84,
                pct_10: 13.90,
                pct_20: 8.75,
                pct_30: 6.83,
            }),
            StockYogoSizeRow {
                pct_10: 38.08,
                pct_15: 20.60,
                pct_20: 14.65,
                pct_25: 11.58,
            },
        ),
        16 => (
            Some(StockYogoBiasRow {
                pct_5: 23.87,
                pct_10: 14.44,
                pct_20: 9.00,
                pct_30: 6.99,
            }),
            StockYogoSizeRow {
                pct_10: 39.80,
                pct_15: 21.48,
                pct_20: 15.24,
                pct_25: 12.03,
            },
        ),
        17 => (
            Some(StockYogoBiasRow {
                pct_5: 24.89,
                pct_10: 14.97,
                pct_20: 9.25,
                pct_30: 7.15,
            }),
            StockYogoSizeRow {
                pct_10: 41.51,
                pct_15: 22.35,
                pct_20: 15.83,
                pct_25: 12.49,
            },
        ),
        18 => (
            Some(StockYogoBiasRow {
                pct_5: 25.91,
                pct_10: 15.50,
                pct_20: 9.49,
                pct_30: 7.30,
            }),
            StockYogoSizeRow {
                pct_10: 43.22,
                pct_15: 23.22,
                pct_20: 16.42,
                pct_25: 12.94,
            },
        ),
        19 => (
            Some(StockYogoBiasRow {
                pct_5: 26.92,
                pct_10: 16.02,
                pct_20: 9.73,
                pct_30: 7.45,
            }),
            StockYogoSizeRow {
                pct_10: 44.92,
                pct_15: 24.09,
                pct_20: 17.02,
                pct_25: 13.39,
            },
        ),
        20 => (
            Some(StockYogoBiasRow {
                pct_5: 27.93,
                pct_10: 16.54,
                pct_20: 9.97,
                pct_30: 7.60,
            }),
            StockYogoSizeRow {
                pct_10: 46.62,
                pct_15: 24.96,
                pct_20: 17.61,
                pct_25: 13.84,
            },
        ),
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias, size })
}

/// Stock-Yogo (2005) LIML size of nominal 5% Wald test. k2=排除工具数。
/// 来源: ivreg2.ado cdsy type(limlsize10|15|20|25). LIML 无 bias 行。
pub(super) fn stock_yogo_cv_liml_1_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let size = match k2 {
        1 => StockYogoSizeRow {
            pct_10: 16.38,
            pct_15: 8.96,
            pct_20: 6.66,
            pct_25: 5.53,
        },
        2 => StockYogoSizeRow {
            pct_10: 8.68,
            pct_15: 5.33,
            pct_20: 4.42,
            pct_25: 3.92,
        },
        3 => StockYogoSizeRow {
            pct_10: 6.46,
            pct_15: 4.36,
            pct_20: 3.69,
            pct_25: 3.32,
        },
        4 => StockYogoSizeRow {
            pct_10: 5.44,
            pct_15: 3.87,
            pct_20: 3.30,
            pct_25: 2.98,
        },
        5 => StockYogoSizeRow {
            pct_10: 4.84,
            pct_15: 3.56,
            pct_20: 3.05,
            pct_25: 2.77,
        },
        6 => StockYogoSizeRow {
            pct_10: 4.45,
            pct_15: 3.34,
            pct_20: 2.87,
            pct_25: 2.61,
        },
        7 => StockYogoSizeRow {
            pct_10: 4.18,
            pct_15: 3.18,
            pct_20: 2.73,
            pct_25: 2.49,
        },
        8 => StockYogoSizeRow {
            pct_10: 3.97,
            pct_15: 3.04,
            pct_20: 2.63,
            pct_25: 2.39,
        },
        9 => StockYogoSizeRow {
            pct_10: 3.81,
            pct_15: 2.93,
            pct_20: 2.54,
            pct_25: 2.32,
        },
        10 => StockYogoSizeRow {
            pct_10: 3.68,
            pct_15: 2.84,
            pct_20: 2.46,
            pct_25: 2.25,
        },
        11 => StockYogoSizeRow {
            pct_10: 3.58,
            pct_15: 2.76,
            pct_20: 2.40,
            pct_25: 2.19,
        },
        12 => StockYogoSizeRow {
            pct_10: 3.50,
            pct_15: 2.69,
            pct_20: 2.34,
            pct_25: 2.14,
        },
        13 => StockYogoSizeRow {
            pct_10: 3.42,
            pct_15: 2.63,
            pct_20: 2.29,
            pct_25: 2.10,
        },
        14 => StockYogoSizeRow {
            pct_10: 3.36,
            pct_15: 2.57,
            pct_20: 2.25,
            pct_25: 2.06,
        },
        15 => StockYogoSizeRow {
            pct_10: 3.31,
            pct_15: 2.52,
            pct_20: 2.21,
            pct_25: 2.03,
        },
        16 => StockYogoSizeRow {
            pct_10: 3.27,
            pct_15: 2.48,
            pct_20: 2.18,
            pct_25: 2.00,
        },
        17 => StockYogoSizeRow {
            pct_10: 3.24,
            pct_15: 2.44,
            pct_20: 2.14,
            pct_25: 1.97,
        },
        18 => StockYogoSizeRow {
            pct_10: 3.20,
            pct_15: 2.41,
            pct_20: 2.11,
            pct_25: 1.94,
        },
        19 => StockYogoSizeRow {
            pct_10: 3.18,
            pct_15: 2.37,
            pct_20: 2.09,
            pct_25: 1.92,
        },
        20 => StockYogoSizeRow {
            pct_10: 3.21,
            pct_15: 2.34,
            pct_20: 2.06,
            pct_25: 1.90,
        },
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias: None, size })
}

/// Stock-Yogo (2005) LIML size of nominal 5% Wald test，2 内生变量。
pub(super) fn stock_yogo_cv_liml_2_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let size = match k2 {
        2 => StockYogoSizeRow {
            pct_10: 7.03,
            pct_15: 4.58,
            pct_20: 3.95,
            pct_25: 3.63,
        },
        3 => StockYogoSizeRow {
            pct_10: 5.44,
            pct_15: 3.81,
            pct_20: 3.32,
            pct_25: 3.09,
        },
        4 => StockYogoSizeRow {
            pct_10: 4.72,
            pct_15: 3.39,
            pct_20: 2.99,
            pct_25: 2.79,
        },
        5 => StockYogoSizeRow {
            pct_10: 4.32,
            pct_15: 3.13,
            pct_20: 2.78,
            pct_25: 2.60,
        },
        6 => StockYogoSizeRow {
            pct_10: 4.06,
            pct_15: 2.95,
            pct_20: 2.63,
            pct_25: 2.46,
        },
        7 => StockYogoSizeRow {
            pct_10: 3.90,
            pct_15: 2.83,
            pct_20: 2.52,
            pct_25: 2.35,
        },
        8 => StockYogoSizeRow {
            pct_10: 3.78,
            pct_15: 2.73,
            pct_20: 2.43,
            pct_25: 2.27,
        },
        9 => StockYogoSizeRow {
            pct_10: 3.70,
            pct_15: 2.66,
            pct_20: 2.36,
            pct_25: 2.20,
        },
        10 => StockYogoSizeRow {
            pct_10: 3.64,
            pct_15: 2.60,
            pct_20: 2.30,
            pct_25: 2.14,
        },
        11 => StockYogoSizeRow {
            pct_10: 3.60,
            pct_15: 2.55,
            pct_20: 2.25,
            pct_25: 2.09,
        },
        12 => StockYogoSizeRow {
            pct_10: 3.58,
            pct_15: 2.52,
            pct_20: 2.21,
            pct_25: 2.05,
        },
        13 => StockYogoSizeRow {
            pct_10: 3.56,
            pct_15: 2.48,
            pct_20: 2.17,
            pct_25: 2.02,
        },
        14 => StockYogoSizeRow {
            pct_10: 3.55,
            pct_15: 2.46,
            pct_20: 2.14,
            pct_25: 1.99,
        },
        15 => StockYogoSizeRow {
            pct_10: 3.54,
            pct_15: 2.44,
            pct_20: 2.11,
            pct_25: 1.96,
        },
        16 => StockYogoSizeRow {
            pct_10: 3.55,
            pct_15: 2.42,
            pct_20: 2.09,
            pct_25: 1.93,
        },
        17 => StockYogoSizeRow {
            pct_10: 3.55,
            pct_15: 2.41,
            pct_20: 2.07,
            pct_25: 1.91,
        },
        18 => StockYogoSizeRow {
            pct_10: 3.56,
            pct_15: 2.40,
            pct_20: 2.05,
            pct_25: 1.89,
        },
        19 => StockYogoSizeRow {
            pct_10: 3.57,
            pct_15: 2.39,
            pct_20: 2.03,
            pct_25: 1.87,
        },
        20 => StockYogoSizeRow {
            pct_10: 3.58,
            pct_15: 2.38,
            pct_20: 2.02,
            pct_25: 1.86,
        },
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias: None, size })
}
