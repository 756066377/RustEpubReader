use serde_json::Value;
use std::collections::HashMap;

const ZH_CN: &str = include_str!("zh_cn.json");
const EN: &str = include_str!("en.json");

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Language {
    #[default]
    ZhCN,
    En,
}

impl Language {
    pub fn label(&self) -> &'static str {
        match self {
            Language::ZhCN => "中文",
            Language::En => "English",
        }
    }

    pub fn all() -> &'static [Language] {
        &[Language::ZhCN, Language::En]
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::ZhCN => "zh_cn",
            Language::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "en" => Language::En,
            _ => Language::ZhCN,
        }
    }
}

#[derive(Debug, Clone)]
pub struct I18n {
    language: Language,
    translations: HashMap<String, String>,
}

impl I18n {
    pub fn new(language: Language) -> Self {
        let json_str = match language {
            Language::ZhCN => ZH_CN,
            Language::En => EN,
        };
        let translations = Self::parse_json(json_str);
        Self {
            language,
            translations,
        }
    }

    fn parse_json(json_str: &str) -> HashMap<String, String> {
        let value: Value =
            serde_json::from_str(json_str).unwrap_or(Value::Object(Default::default()));
        let mut map = HashMap::new();
        if let Value::Object(obj) = value {
            for (key, val) in obj {
                if let Value::String(s) = val {
                    map.insert(key, s);
                }
            }
        }
        map
    }

    pub fn language(&self) -> &Language {
        &self.language
    }

    pub fn set_language(&mut self, language: Language) {
        if self.language != language {
            self.language = language.clone();
            let json_str = match language {
                Language::ZhCN => ZH_CN,
                Language::En => EN,
            };
            self.translations = Self::parse_json(json_str);
        }
    }

    /// Get a translated string by key. Returns the key itself if not found.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        match self.translations.get(key) {
            Some(s) => s.as_str(),
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[i18n] missing key: {key}");
                key
            }
        }
    }

    /// Get a translated string with a single `{}` placeholder replaced.
    pub fn tf1(&self, key: &str, arg: &str) -> String {
        let template = self.t(key);
        template.replacen("{}", arg, 1)
    }

    /// Get a translated string with two `{}` placeholders replaced.
    pub fn tf2(&self, key: &str, arg1: &str, arg2: &str) -> String {
        let template = self.t(key);
        let s = template.replacen("{}", arg1, 1);
        s.replacen("{}", arg2, 1)
    }

    /// Get all translated key-value pairs
    pub fn get_all_translations(&self) -> &HashMap<String, String> {
        &self.translations
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Language::default())
    }
}
