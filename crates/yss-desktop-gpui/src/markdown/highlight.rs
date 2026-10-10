use gpui_kit::component::highlighter::{GrammarConfig, LanguageRegistry};

pub(crate) fn init() {
    let registry = LanguageRegistry::singleton();

    // Map upstream capture names to the component's theme vocabulary.
    let r_highlights = tree_sitter_r::HIGHLIGHTS_QUERY
        .replace("@conditional", "@keyword.conditional")
        .replace("@repeat", "@keyword.repeat")
        .replace("@namespace", "@type");
    let r = GrammarConfig::new(
        "r",
        tree_sitter_r::LANGUAGE.into(),
        vec![],
        &r_highlights,
        "",
        tree_sitter_r::LOCALS_QUERY,
    );
    for name in ["r", "R"] {
        registry.register(name, &r);
    }

    let julia_highlights = arborium_julia::HIGHLIGHTS_QUERY.replace("@character", "@string");
    let julia = GrammarConfig::new(
        "julia",
        arborium_julia::language().into(),
        vec![],
        &julia_highlights,
        arborium_julia::INJECTIONS_QUERY,
        arborium_julia::LOCALS_QUERY,
    );
    for name in ["julia", "jl"] {
        registry.register(name, &julia);
    }
}
