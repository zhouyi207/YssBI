//! Small native vector assets composed with the component library's existing bundle.
use anyhow::Result;
use gpui::{AssetSource, SharedString};
use gpui_component::IconNamed;
use std::borrow::Cow;

pub struct Assets;

const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/yssbi/chart.svg",
        include_bytes!("../assets/icons/chart.svg"),
    ),
    (
        "icons/yssbi/save.svg",
        include_bytes!("../assets/icons/save.svg"),
    ),
    (
        "icons/yssbi/play.svg",
        include_bytes!("../assets/icons/play.svg"),
    ),
    (
        "icons/yssbi/stop.svg",
        include_bytes!("../assets/icons/stop.svg"),
    ),
    (
        "icons/yssbi/graph.svg",
        include_bytes!("../assets/icons/graph.svg"),
    ),
    (
        "icons/yssbi/database.svg",
        include_bytes!("../assets/icons/database.svg"),
    ),
    (
        "icons/yssbi/table.svg",
        include_bytes!("../assets/icons/table.svg"),
    ),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit_assets::AllAssets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_kit_assets::AllAssets.list(path)?;
        assets.extend(
            ICONS
                .iter()
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::from(*name)),
        );
        Ok(assets)
    }
}

#[derive(Clone, Copy)]
pub enum NativeIcon {
    Chart,
    Save,
    Play,
    Stop,
    Graph,
    Database,
    Table,
}
impl IconNamed for NativeIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Chart => "icons/yssbi/chart.svg",
            Self::Save => "icons/yssbi/save.svg",
            Self::Play => "icons/yssbi/play.svg",
            Self::Stop => "icons/yssbi/stop.svg",
            Self::Graph => "icons/yssbi/graph.svg",
            Self::Database => "icons/yssbi/database.svg",
            Self::Table => "icons/yssbi/table.svg",
        }
        .into()
    }
}
