//! LaTeX typesetting runs with Markdown parsing, off the UI thread.
use std::{
    num::NonZeroUsize,
    sync::{Arc, LazyLock, Mutex},
};

use lru::LruCache;
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_svg::{SvgColorSyntax, SvgOptions, render_to_svg_with_color_syntax};
use ratex_types::math_style::MathStyle;

/// SVG geometry in em units, including a small inset for glyph overhangs.
pub(super) struct Formula {
    pub svg: Vec<u8>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

type Prepared = Option<Arc<Formula>>;
type Cache = LruCache<(String, bool), Prepared>;

// Markdown probes inline plugins before converting their nodes. Reuse that work,
// including failures, and bound retained formula data across documents/messages.
static CACHE: LazyLock<Mutex<Cache>> =
    LazyLock::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(128).unwrap())));

pub(super) fn prepare(latex: &str, display: bool) -> Prepared {
    let key = (latex.to_owned(), display);
    if let Ok(mut cache) = CACHE.lock()
        && let Some(formula) = cache.get(&key)
    {
        return formula.clone();
    }
    let formula = typeset(latex, display).map(Arc::new);
    if let Ok(mut cache) = CACHE.lock() {
        cache.put(key, formula.clone());
    }
    formula
}

fn typeset(latex: &str, display: bool) -> Option<Formula> {
    let nodes = ratex_parser::parse(latex).ok()?;
    let options = LayoutOptions::default().with_style(if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    });
    let list = to_display_list(&layout(&nodes, &options));
    const EM: f64 = 40.;
    const INSET: f64 = 0.05;
    let width = list.width + 2. * INSET;
    let height = list.height + list.depth + 2. * INSET;
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return None;
    }
    let svg = render_to_svg_with_color_syntax(
        &list,
        &SvgOptions {
            font_size: EM,
            padding: EM * INSET,
            embed_glyphs: true,
            ..Default::default()
        },
        SvgColorSyntax::Rgb,
    );
    Some(Formula {
        svg: svg.into_bytes(),
        width: width as f32,
        height: height as f32,
        baseline: (list.height + INSET) as f32,
    })
}
