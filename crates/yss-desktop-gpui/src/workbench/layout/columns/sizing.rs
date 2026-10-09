//! Keep the root DockArea's conversation widths while editor slots absorb available space.
use super::conversation_node;
use gpui::{App, Axis, Pixels, WeakEntity, Window, px};
use gpui_base::PANEL_MIN_SIZE;
use gpui_component::dock::{DockArea, DockPlacement, NodeId, PaneRef};

pub(super) fn default_width(dock: &DockArea, window: &Window) -> Pixels {
    let width = if dock.bounds().size.width > px(0.) {
        dock.bounds().size.width
    } else {
        window.viewport_size().width
    };
    let sides = [DockPlacement::Left, DockPlacement::Right]
        .into_iter()
        .filter(|side| dock.is_dock_open(*side))
        .filter_map(|side| dock.dock_size(side))
        .fold(px(0.), |total, width| total + width);
    px(420.).min(((width - sides) * 0.45).max(PANEL_MIN_SIZE))
}

pub(in crate::workbench) fn fit(
    area: &WeakEntity<DockArea>,
    node: NodeId,
    width: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    if area
        .upgrade()
        .is_none_or(|area| fitted_sizes(area.read(cx), node, width).is_none())
    {
        return;
    }
    let area = area.clone();
    // Base rescales its measured split sizes during prepaint. Apply the pixel
    // widths after it has measured the new container, before the settling frame.
    window.defer(cx, move |window, cx| {
        let _ = area.update(cx, |dock, cx| {
            if let Some(sizes) = fitted_sizes(dock, node, width) {
                dock.set_split_sizes(node, sizes, window, cx);
            }
        });
    });
}

fn fitted_sizes(dock: &DockArea, node: NodeId, width: Pixels) -> Option<Vec<Pixels>> {
    let tree = dock.layout(DockPlacement::Center)?;
    if dock.is_zoomed() || tree.root().id() != node {
        return None;
    }
    let PaneRef::Split {
        axis: Axis::Horizontal,
        children,
        sizes,
    } = tree.root().kind()
    else {
        return None;
    };
    let leading = children
        .iter()
        .take_while(|child| conversation_node(dock, child))
        .count();
    if leading == 0 || leading == children.len() || sizes[..leading].iter().any(Option::is_none) {
        return None;
    }
    let mut fitted: Vec<_> = sizes
        .iter()
        .map(|size| size.unwrap_or(PANEL_MIN_SIZE).max(PANEL_MIN_SIZE))
        .collect();
    let fixed: Pixels = fitted[..leading].iter().copied().sum();
    let editors = &mut fitted[leading..];
    let available = width - fixed;
    let minimum = PANEL_MIN_SIZE * editors.len() as f32;
    // Let native minimum-size constraints handle windows too narrow for the
    // requested widths; do not overwrite the user's widths with that constraint.
    if available < minimum {
        return None;
    }
    let previous_extra: Pixels = editors.iter().map(|size| *size - PANEL_MIN_SIZE).sum();
    let equal_share = 1. / editors.len() as f32;
    for size in editors {
        let share = if previous_extra > px(0.) {
            (*size - PANEL_MIN_SIZE) / previous_extra
        } else {
            equal_share
        };
        *size = PANEL_MIN_SIZE + (available - minimum) * share;
    }
    sizes
        .iter()
        .zip(&fitted)
        .any(|(before, after)| before.is_none_or(|before| (before - *after).as_f32().abs() > 0.25))
        .then_some(fitted)
}
