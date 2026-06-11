use serde::{Deserialize, Serialize};

/// 支持的语言
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub code: String,
    pub name: String,
}

impl Language {
    pub fn supported() -> Vec<Self> {
        vec![
            Language { code: "zh".into(), name: "中文".into() },
            Language { code: "en".into(), name: "English".into() },
            Language { code: "yue".into(), name: "粤语".into() },
            Language { code: "ja".into(), name: "日本語".into() },
            Language { code: "ko".into(), name: "한국어".into() },
        ]
    }

    #[allow(dead_code)]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::supported().into_iter().find(|l| l.code == code)
    }
}