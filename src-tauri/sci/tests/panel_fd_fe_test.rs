//! Test: For T=2, FD and FE should give identical slope coefficients.

use ndarray::{Array1, Array2};
use yss_sci::regression::panel::{fit_panel_fe, fit_panel_fd};

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
    let fd = fit_panel_fd(&endog, &exog, &entity_id, &time_id, true, "nonrobust", None).unwrap();

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
