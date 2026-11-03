//! Vietnamese / English glosses shown beside Chinese candidates (never inserted).
use std::{collections::HashMap, path::Path, sync::OnceLock};

#[derive(Default)]
pub struct Meanings(HashMap<String, HashMap<String, String>>);

static SHARED: OnceLock<Meanings> = OnceLock::new();

impl Meanings {
    pub fn parse(json: &[u8]) -> Meanings {
        Meanings(serde_json::from_slice(json).unwrap_or_default())
    }

    /// Loads the large immutable dictionary once and shares it across input contexts.
    pub fn load_shared(path: &Path) -> &'static Meanings {
        SHARED.get_or_init(|| Meanings::parse(&std::fs::read(path).unwrap_or_default()))
    }

    pub fn shared() -> Option<&'static Meanings> {
        SHARED.get()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn gloss(&self, word: &str, language: &str) -> Option<&str> {
        self.0
            .get(word)?
            .get(language)
            .map(String::as_str)
            .filter(|gloss| !gloss.is_empty())
    }

    /// Keep whole-word glosses; otherwise annotate individual words/characters, not a
    /// sentence translation. `language` is "vi" or "en".
    pub fn annotate(&self, candidate: &str, language: &str) -> Option<String> {
        if let Some(meaning) = self.gloss(candidate, language) {
            return Some(short(meaning));
        }
        let characters: Vec<&str> = candidate
            .char_indices()
            .map(|(i, c)| &candidate[i..i + c.len_utf8()])
            .collect();
        let mut parts = Vec::new();
        let mut offset = 0;
        while offset < characters.len() {
            // Longest dictionary word first; a single character is the final fallback.
            let found = (1..=characters.len() - offset).rev().find_map(|length| {
                let word = characters[offset..offset + length].concat();
                let gloss = self.gloss(&word, language)?;
                // Short first senses keep sentence annotations readable while typing.
                Some((length, format!("{word}: {}", short(gloss))))
            });
            match found {
                Some((length, part)) => {
                    parts.push(part);
                    offset += length;
                }
                None => offset += 1, // Unknown characters/punctuation have no placeholder.
            }
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

fn short(gloss: &str) -> String {
    let first = gloss.split(';').next().unwrap_or_default().trim();
    if first.encode_utf16().count() <= 72 {
        return first.to_owned();
    }
    let mut units = 0;
    let mut result: String = first
        .chars()
        .take_while(|c| {
            let fits = units < 69;
            units += c.len_utf16();
            fits
        })
        .collect();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_annotations() {
        let meanings = Meanings::parse(
            r#"{"我": {"en": "I; me", "vi": "tôi"}, "学习": {"en": "study", "vi": "học"},
                "中文": {"en": "Chinese", "vi": "tiếng Trung"}, "学": {"en": "learn"},
                "𠮷": {"en": "lucky"}}"#
                .as_bytes(),
        );
        assert_eq!(
            meanings.annotate("我学习中文！", "en").as_deref(),
            Some("我: I · 学习: study · 中文: Chinese")
        );
        assert_eq!(
            meanings.annotate("𠮷🫠我", "en").as_deref(),
            Some("𠮷: lucky · 我: I")
        );
        assert_eq!(
            meanings.annotate("我学习中文", "vi").as_deref(),
            Some("我: tôi · 学习: học · 中文: tiếng Trung")
        );
        assert_eq!(meanings.annotate("🫠", "en"), None);
        assert_eq!(meanings.annotate("学", "vi"), None);
    }

    #[test]
    fn long_glosses_are_shortened() {
        let gloss = "x".repeat(80);
        let shortened = short(&gloss);
        assert_eq!(shortened.chars().count(), 70);
        assert!(shortened.ends_with('…'));
        assert_eq!(short(" first sense ; second"), "first sense");
    }

    #[test]
    fn bundled_dictionary() {
        let meanings = Meanings::parse(include_bytes!("../data/chinese-meanings.json"));
        assert!(meanings.len() > 120_000);
        for word in ["图书馆", "环境", "经济", "人工智能", "旅游", "医院"] {
            assert!(meanings.gloss(word, "vi").is_some() && meanings.gloss(word, "en").is_some());
        }
        assert_eq!(meanings.annotate("你好", "vi").as_deref(), Some("xin chào"));
        assert_eq!(meanings.annotate("你好", "en").as_deref(), Some("hello"));
    }
}
