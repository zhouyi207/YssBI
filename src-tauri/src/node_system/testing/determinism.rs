use std::fmt::Debug;

/// Runs locale switches against mutable test state and verifies that its
/// canonical document snapshot never changes.
#[track_caller]
pub fn assert_locale_invariance<State, Snapshot>(
    state: &mut State,
    locales: &[&str],
    snapshot: impl Fn(&State) -> Snapshot,
    mut switch_locale: impl FnMut(&mut State, &str),
) where
    Snapshot: Debug + Eq,
{
    let expected = snapshot(state);
    for locale in locales {
        switch_locale(state, locale);
        assert_eq!(
            snapshot(state),
            expected,
            "locale '{locale}' changed locale-independent state"
        );
    }
}

/// Replays the same insertion set in deterministic pseudo-random orders and
/// compares a caller-provided semantic snapshot for every trial.
#[track_caller]
pub fn assert_random_insertion_order_determinism<Item>(
    items: &[Item],
    seed: u64,
    trials: usize,
    snapshot_for_order: impl Fn(&[Item]) -> String,
) where
    Item: Clone,
{
    assert!(
        trials > 0,
        "determinism harness requires at least one trial"
    );
    let expected = snapshot_for_order(items);
    let mut state = seed.max(1);

    for trial in 0..trials {
        let mut shuffled = items.to_vec();
        for index in (1..shuffled.len()).rev() {
            state = xorshift64(state);
            shuffled.swap(index, state as usize % (index + 1));
        }
        assert_eq!(
            snapshot_for_order(&shuffled),
            expected,
            "semantic snapshot changed in randomized insertion trial {trial}"
        );
    }
}

fn xorshift64(mut value: u64) -> u64 {
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    value
}
