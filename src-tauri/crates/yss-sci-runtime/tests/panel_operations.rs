//! Panel alignment and differencing.
use yss_sci_runtime::data::panel::{align_panel, panel_diff};

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
