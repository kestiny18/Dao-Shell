//! Explicit language preference shared with desktop configuration. CLI stays Chinese.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiLanguage {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Chinese,
    English,
}
impl Language {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
        }
    }
    pub fn choose<'a>(self, chinese: &'a str, english: &'a str) -> &'a str {
        match self {
            Self::Chinese => chinese,
            Self::English => english,
        }
    }
}
impl UiLanguage {
    /// A Windows LANGID uses the low ten bits for the primary language.
    pub fn resolve(self, windows_ui_language: Option<u16>) -> Language {
        match self {
            Self::Chinese => Language::Chinese,
            Self::English => Language::English,
            Self::Auto if windows_ui_language.is_some_and(|id| id & 0x03ff == 0x04) => {
                Language::Chinese
            }
            Self::Auto => Language::English,
        }
    }
}
/// User UI language, deliberately independent of region, time zone and formatting locale.
pub fn system_ui_language() -> Option<u16> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        let id = unsafe { GetUserDefaultUILanguage() };
        (id != 0).then_some(id)
    }
    #[cfg(not(windows))]
    {
        None
    }
}
pub fn effective(preference: UiLanguage) -> Language {
    preference.resolve(system_ui_language())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ui_language_is_not_region_and_overrides_are_explicit() {
        for id in [0x0804, 0x0404, 0x0c04, 0x1004, 0x1404] {
            assert_eq!(UiLanguage::Auto.resolve(Some(id)), Language::Chinese);
        }
        for id in [None, Some(0), Some(0x0409), Some(0x0411), Some(0x0407)] {
            assert_eq!(UiLanguage::Auto.resolve(id), Language::English);
        }
        assert_eq!(UiLanguage::Chinese.resolve(Some(0x0409)), Language::Chinese);
        assert_eq!(UiLanguage::English.resolve(Some(0x0804)), Language::English);
        assert_eq!(
            serde_json::from_str::<UiLanguage>("\"auto\"").unwrap(),
            UiLanguage::Auto
        );
        assert!(serde_json::from_str::<UiLanguage>("\"fr\"").is_err());
    }
}
