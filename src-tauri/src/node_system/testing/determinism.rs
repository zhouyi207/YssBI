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
