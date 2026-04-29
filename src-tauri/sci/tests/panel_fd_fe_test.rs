//! Test: For T=2, FD and FE should give identical slope coefficients.
//! Panel align + diff used by FD.
//!
//! Stata equivalence: reg D.y D.x, nocons (after xtset id time).
//! D. = first difference = current - L. (previous period within panel).

use ndarray::{Array1, Array2};
use yss_sci::panel::align::{align_panel, panel_diff};
use yss_sci::regression::panel::{fit_panel_fe, fit_panel_fd, fit_panel_lsdv};

#[test]
fn test_fd_fe_identical_for_t2() {
    // T=2, N=3 entities -> 6 obs. y = 1 + 2*x + entity_effect + noise
    // Entity 0: (t0,y0,x0), (t1,y1,x1)
    // Entity 1: (t0,y0,x0), (t1,y1,x1)
    // Entity 2: (t0,y0,x0), (t1,y1,x1)
    let endog = Array1::from_vec(vec![
        3.0, 7.0,   // entity 0: y0=1+2*1=3, y1=1+2*3=7
        5.0, 9.0,   // entity 1: y0=1+2*2=5, y1=1+2*4=9
        7.0, 11.0,  // entity 2: y0=1+2*3=7, y1=1+2*5=11
    ]);
    let exog = Array2::from_shape_vec(
        (6, 2),
        vec![
            1.0, 1.0,  // entity 0 t0: const, x
            1.0, 3.0,  // entity 0 t1
            1.0, 2.0,  // entity 1 t0
            1.0, 4.0,  // entity 1 t1
            1.0, 3.0,  // entity 2 t0
            1.0, 5.0,  // entity 2 t1
        ],
    )
    .unwrap();
    let entity_id: Vec<usize> = vec![0, 0, 1, 1, 2, 2];
    let time_id: Vec<usize> = vec![0, 1, 0, 1, 0, 1];

    let fe = fit_panel_fe(&endog, &exog, &entity_id, true, "nonrobust", None).unwrap();
    let time_values: Vec<i64> = time_id.iter().map(|&t| t as i64).collect();
    let fd = fit_panel_fd(&endog, &exog, &entity_id, &time_id, &time_values, true, "nonrobust", None).unwrap();

    // FE: betas[0]=const, betas[1]=slope. FD: betas[0]=slope (no const)
    let fe_slope = fe.betas[1];
    let fd_slope = fd.betas[0];

    assert!(
        (fe_slope - fd_slope).abs() < 1e-10,
        "T=2: FE slope {} should equal FD slope {}",
        fe_slope,
        fd_slope
    );
    assert!(
        (fe_slope - 2.0).abs() < 1e-10,
        "Expected slope 2, got FE {}",
        fe_slope
    );
}

#[test]
fn test_panel_align_diff() {
    // Entity 0: t=0,1,2; Entity 1: t=0,2 (gap at t=1)
    let entity_id = vec![0, 0, 0, 1, 1];
    let time_id = vec![0, 1, 2, 0, 2];
    let col = vec![10.0, 20.0, 30.0, 5.0, 25.0]; // entity 0: 10,20,30; entity 1: 5,25

    let aligned = align_panel(&entity_id, &time_id, &[col.clone()], Some(1)).unwrap();
    // Entity 0: full grid 0,1,2 -> 10,20,30
    // Entity 1: full grid 0,1,2 -> 5, NaN, 25
    assert_eq!(aligned.entity_id.len(), 6);
    assert_eq!(aligned.entity_id, vec![0, 0, 0, 1, 1, 1]);
    assert_eq!(aligned.time_id, vec![0, 1, 2, 0, 1, 2]);

    let (diff_entity, _diff_time_id, diff_cols) = panel_diff(&aligned).unwrap();
    // Entity 0: diff(20-10), diff(30-20) -> 2 obs
    // Entity 1: diff(25-5) (skip NaN) -> 1 obs
    assert_eq!(diff_entity, vec![0, 0, 1]);
    assert_eq!(diff_cols[0], vec![10.0, 10.0, 20.0]);
}

/// T=3: FD slope should equal true slope when y = 1 + 2*x (no FE in diff).
/// Stata: reg D.y D.x, nocons
#[test]
fn test_fd_slope_t3() {
    // 2 entities, 3 periods each. y = 1 + 2*x
    // Entity 0: x=(1,2,3) -> y=(3,5,7); Entity 1: x=(2,4,6) -> y=(5,9,13)
    // Δy = 2*Δx exactly
    let endog = Array1::from_vec(vec![
        3.0, 5.0, 7.0,   // entity 0
        5.0, 9.0, 13.0,  // entity 1
    ]);
    let exog = Array2::from_shape_vec(
        (6, 2),
        vec![
            1.0, 1.0, 1.0, 1.0, 1.0, 1.0,  // const (all 1s)
            1.0, 2.0, 3.0, 2.0, 4.0, 6.0,  // x
        ],
    )
    .unwrap();
    let entity_id = vec![0, 0, 0, 1, 1, 1];
    let time_id = vec![0, 1, 2, 0, 1, 2];

    let time_values: Vec<i64> = time_id.iter().map(|&t| t as i64).collect();
    let fd = fit_panel_fd(&endog, &exog, &entity_id, &time_id, &time_values, true, "nonrobust", None).unwrap();
    let slope = fd.betas[0];
    assert!(
        (slope - 2.0).abs() < 1e-10,
        "FD slope should be 2.0, got {}",
        slope
    );
    assert_eq!(fd.num_observation, 4, "4 FD obs (2 per entity)");
}

/// Time gaps: 与 Stata 一致，仅对相邻时间点（delta=1）差分，不跨 gap。
/// Entity 1 有 t=0,2（gap），不产生 FD 观测。
#[test]
fn test_fd_with_time_gap() {
    // Entity 0: t=0,1,2 (full); Entity 1: t=0,2 (gap at t=1)
    // y = 2*x: Entity 0: (0,1,2)->(0,2,4); Entity 1: (0,2)->(0,4)
    let endog = Array1::from_vec(vec![0.0, 2.0, 4.0, 0.0, 4.0]);
    // Row-major (5,2): row i = (exog[[i,0]], exog[[i,1]])
    let exog = Array2::from_shape_vec(
        (5, 2),
        vec![
            1.0, 0.0,  // row 0: e0,t0
            1.0, 1.0,  // row 1: e0,t1
            1.0, 2.0,  // row 2: e0,t2
            1.0, 0.0,  // row 3: e1,t0
            1.0, 2.0,  // row 4: e1,t2
        ],
    )
    .unwrap();
    let entity_id = vec![0, 0, 0, 1, 1];
    let time_id = vec![0, 1, 2, 0, 2];  // entity 1 has gap at t=1

    let time_values: Vec<i64> = time_id.iter().map(|&t| t as i64).collect();
    let fd = fit_panel_fd(&endog, &exog, &entity_id, &time_id, &time_values, true, "nonrobust", None).unwrap();
    // Entity 0: Δy=(2,2), Δx=(1,1) -> slope 2
    // Entity 1: 不产生 FD（gap 2-0≠1）
    let slope = fd.betas[0];
    assert!(
        (slope - 2.0).abs() < 1e-10,
        "FD slope with gap should be 2.0, got {}",
        slope
    );
    assert_eq!(fd.num_observation, 2, "2 FD obs (from e0 only, e1 skipped due to gap)");
}

#[test]
fn test_lsdv_matches_fe_slope() {
    let endog = Array1::from_vec(vec![
        3.0, 5.0, 7.0,   // entity 0: effect 0, y = 1 + 2x
        7.0, 11.0, 15.0, // entity 1: effect 2, y = 3 + 2x
        11.0, 17.0, 23.0, // entity 2: effect 4, y = 5 + 2x
    ]);
    let exog = Array2::from_shape_vec(
        (9, 2),
        vec![
            1.0, 1.0,
            1.0, 2.0,
            1.0, 3.0,
            1.0, 2.0,
            1.0, 4.0,
            1.0, 6.0,
            1.0, 3.0,
            1.0, 6.0,
            1.0, 9.0,
        ],
    )
    .unwrap();
    let entity_id = vec![0, 0, 0, 1, 1, 1, 2, 2, 2];

    let fe = fit_panel_fe(&endog, &exog, &entity_id, true, "nonrobust", None).unwrap();
    let lsdv = fit_panel_lsdv(&endog, &exog, &entity_id, true, "nonrobust", None).unwrap();

    assert!((fe.betas[1] - 2.0).abs() < 1e-10);
    assert!((lsdv.betas[1] - 2.0).abs() < 1e-10);
    assert!((fe.betas[1] - lsdv.betas[1]).abs() < 1e-10);
    assert_eq!(lsdv.num_entities, 3);
    assert_eq!(lsdv.num_observation, 9);
}

#[test]
fn test_fe_cluster_reports_stata_style_stats() {
    let endog = Array1::from_vec(vec![
        3.0, 5.1, 6.9,
        7.0, 11.2, 14.8,
        11.0, 16.9, 23.1,
        13.0, 21.1, 29.0,
    ]);
    let exog = Array2::from_shape_vec(
        (12, 2),
        vec![
            1.0, 1.0,
            1.0, 2.0,
            1.0, 3.0,
            1.0, 2.0,
            1.0, 4.0,
            1.0, 6.0,
            1.0, 3.0,
            1.0, 6.0,
            1.0, 9.0,
            1.0, 4.0,
            1.0, 8.0,
            1.0, 12.0,
        ],
    )
    .unwrap();
    let entity_id = vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];

    let fe = fit_panel_fe(&endog, &exog, &entity_id, true, "cluster", None).unwrap();

    assert_eq!(fe.covariance_type, "cluster");
    assert_eq!(fe.num_entities, 4);
    assert!(fe.fe_stats.is_some());
    assert!(fe.fvalue.is_finite());
    assert!(fe.f_p_value.is_finite());
    assert!(fe.cov_beta_nonrobust.is_some());
    assert!((fe.betas[1] - 2.0).abs() < 0.05);
}
