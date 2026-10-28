//! UI strings and colors from Registrator Viewer translation files (`key  "value"` per line).

use std::collections::HashMap;

use egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::Russian];

    fn source(self) -> &'static str {
        match self {
            Language::English => include_str!("../../../assets/lang/en.txt"),
            Language::Russian => include_str!("../../../assets/lang/ru.txt"),
        }
    }
}

pub struct Strings {
    pub language: Language,
    map: HashMap<String, String>,
}

impl Strings {
    pub fn load(language: Language) -> Self {
        Self {
            language,
            map: parse(language.source()),
        }
    }

    /// Translated text for `key`; the key itself when missing.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.map.get(key).map_or(key, String::as_str)
    }

    pub fn native_name(&self) -> &str {
        self.t("LangName")
    }

    /// `#RRGGBB` color entries such as `colorGraph_S`.
    pub fn color(&self, key: &str, fallback: Color32) -> Color32 {
        self.map
            .get(key)
            .and_then(|v| v.strip_prefix('#'))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map_or(fallback, |rgb| {
                Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
            })
    }
}

fn parse(text: &str) -> HashMap<String, String> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (key, rest) = line.split_once(char::is_whitespace)?;
            let value = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
            Some((
                key.to_string(),
                value
                    .replace("&&", "\u{0}")
                    .replace('&', "")
                    .replace('\u{0}', "&"),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys_values_and_colors() {
        let s = Strings {
            language: Language::English,
            map: parse("\u{feff}N_File   \"&File\"\ncolorGraph_S \"#0000FF\"\nbad line\n"),
        };
        assert_eq!(s.t("N_File"), "File");
        assert_eq!(s.t("missing"), "missing");
        assert_eq!(
            s.color("colorGraph_S", Color32::RED),
            Color32::from_rgb(0, 0, 255)
        );
    }

    #[test]
    fn bundled_languages_have_menu_keys() {
        for lang in Language::ALL {
            let s = Strings::load(lang);
            for key in [
                "N_File",
                "N_File_Open",
                "N_View_FileList",
                "Name",
                "Duration",
                "DateTime",
            ] {
                assert!(s.map.contains_key(key), "{lang:?} misses {key}");
            }
        }
    }
}
