use serde::de::{Error as _, IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;

/// 受持久化管理的窗口种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Main,
    DatabaseEditor,
    SourceInspector,
    Logs,
    Plot,
    Info,
    Bayes,
}

#[derive(Debug, Clone, Copy)]
struct WindowKindDescriptor {
    kind: WindowKind,
    key: &'static str,
    default_width: u32,
    default_height: u32,
}

const WINDOW_KIND_DESCRIPTORS: [WindowKindDescriptor; 7] = [
    WindowKindDescriptor {
        kind: WindowKind::Main,
        key: "main",
        default_width: 1600,
        default_height: 900,
    },
    WindowKindDescriptor {
        kind: WindowKind::DatabaseEditor,
        key: "databaseEditor",
        default_width: 1000,
        default_height: 600,
    },
    WindowKindDescriptor {
        kind: WindowKind::SourceInspector,
        key: "sourceInspector",
        default_width: 1000,
        default_height: 600,
    },
    WindowKindDescriptor {
        kind: WindowKind::Logs,
        key: "logs",
        default_width: 1000,
        default_height: 600,
    },
    WindowKindDescriptor {
        kind: WindowKind::Plot,
        key: "plot",
        default_width: 960,
        default_height: 800,
    },
    WindowKindDescriptor {
        kind: WindowKind::Info,
        key: "info",
        default_width: 960,
        default_height: 800,
    },
    WindowKindDescriptor {
        kind: WindowKind::Bayes,
        key: "bayes",
        default_width: 960,
        default_height: 800,
    },
];

impl WindowKind {
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        WINDOW_KIND_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.kind)
    }

    /// 用于命令 wire 与持久化对象 key 的小驼峰字符串。
    pub fn as_str(self) -> &'static str {
        self.descriptor().key
    }

    fn descriptor(self) -> &'static WindowKindDescriptor {
        WINDOW_KIND_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.kind == self)
            .expect("every WindowKind must have a descriptor")
    }

    fn from_key(key: &str) -> Option<Self> {
        WINDOW_KIND_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.key == key)
            .map(|descriptor| descriptor.kind)
    }
}

impl Serialize for WindowKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WindowKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let key = String::deserialize(deserializer)?;
        Self::from_key(&key).ok_or_else(|| D::Error::custom(format!("unknown window kind `{key}`")))
    }
}

/// 单窗口的几何状态。`x/y` 为物理像素坐标，`None` 表示尚未保存过位置。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_maximized: bool,
}

impl WindowState {
    pub(super) fn default_for(kind: WindowKind) -> Self {
        let descriptor = kind.descriptor();
        Self {
            width: descriptor.default_width,
            height: descriptor.default_height,
            x: None,
            y: None,
            is_maximized: false,
        }
    }
}

/// 文件中持久化的整体结构，缺省值表示「尚未保存过」。
#[derive(Debug, Clone, Default)]
pub(super) struct PersistedWindowStates {
    states: HashMap<WindowKind, WindowState>,
}

impl PersistedWindowStates {
    pub(super) fn get(&self, kind: WindowKind) -> Option<&WindowState> {
        self.states.get(&kind)
    }

    pub(super) fn set(&mut self, kind: WindowKind, value: WindowState) {
        self.states.insert(kind, value);
    }
}

impl Serialize for PersistedWindowStates {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(WINDOW_KIND_DESCRIPTORS.len()))?;
        for descriptor in &WINDOW_KIND_DESCRIPTORS {
            map.serialize_entry(descriptor.key, &self.states.get(&descriptor.kind))?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for PersistedWindowStates {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PersistedWindowStatesVisitor;

        impl<'de> Visitor<'de> for PersistedWindowStatesVisitor {
            type Value = PersistedWindowStates;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a window state object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut states = HashMap::with_capacity(WINDOW_KIND_DESCRIPTORS.len());
                while let Some(key) = map.next_key::<String>()? {
                    let Some(kind) = WindowKind::from_key(&key) else {
                        map.next_value::<IgnoredAny>()?;
                        continue;
                    };
                    if let Some(state) = map.next_value::<Option<WindowState>>()? {
                        states.insert(kind, state);
                    }
                }
                Ok(PersistedWindowStates { states })
            }
        }

        deserializer.deserialize_map(PersistedWindowStatesVisitor)
    }
}
