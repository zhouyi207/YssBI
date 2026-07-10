//! 唯一名称生成
//!
//! 与前端 getUniqueName 逻辑一致，避免快速连续创建时重名

use regex::Regex;
use std::collections::HashSet;

/// 根据已有名称列表生成唯一名称
///
/// 规则：baseName 或 baseName N (N 为数字)
/// - 若 baseName 未被占用，返回 baseName
/// - 若 baseName 已占用，返回 baseName 1, baseName 2, ...
pub fn unique_name(base_name: &str, existing: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let escaped = regex::escape(base_name);
    let pattern = format!(r"^{}(?:[ _](\d+))?$", escaped);
    let re = Regex::new(&pattern).expect("regex valid");

    let mut used = HashSet::new();
    let mut has_base = false;

    for name in existing {
        let name = name.as_ref().as_ref();
        if let Some(caps) = re.captures(name) {
            match caps.get(1) {
                Some(m) => {
                    if let Ok(n) = m.as_str().parse::<u32>() {
                        used.insert(n);
                    }
                }
                None => has_base = true,
            }
        }
    }

    if !has_base {
        return base_name.to_string();
    }

    let mut i = 1u32;
    while used.contains(&i) {
        i += 1;
    }
    format!("{} {}", base_name, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_name_empty() {
        assert_eq!(unique_name("New Event", [] as [&str; 0]), "New Event");
    }

    #[test]
    fn test_unique_name_base_exists() {
        assert_eq!(unique_name("New Event", ["New Event"]), "New Event 1");
        assert_eq!(
            unique_name("New Event", ["New Event", "New Event 1"]),
            "New Event 2"
        );
        assert_eq!(
            unique_name("New Event", ["New Event", "New Event_1"]),
            "New Event 2"
        );
    }

    #[test]
    fn test_unique_name_different_base() {
        assert_eq!(unique_name("New Event", ["New Function"]), "New Event");
    }
}
