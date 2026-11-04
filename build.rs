//! Precompute the engine's lookup tables so nothing is parsed at run time:
//! the keyboard starts instantly and the tables live in read-only memory.
use std::{collections::HashMap, env, fmt::Write, fs, path::Path};

#[allow(dead_code)]
mod spelling {
    include!("src/spelling.rs");
    pub fn key(text: &str) -> String {
        spelling_key(text)
    }
    pub fn bases(text: &str) -> String {
        text.chars().map(base).collect()
    }
}

fn main() {
    for file in [
        "src/spelling.rs",
        "data/pinyin_simp.dict.yaml",
        "data/luna_pinyin.dict.yaml",
        "data/rime/keyin-words.tsv",
        "data/vietnamese-syllables.txt",
        "data/english-common.txt",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    let simp = fs::read_to_string("data/pinyin_simp.dict.yaml").unwrap();
    let luna = fs::read_to_string("data/luna_pinyin.dict.yaml").unwrap();
    // Frequent words missing from pinyin_simp, on the same weight scale.
    let words = fs::read_to_string("data/rime/keyin-words.tsv").unwrap();
    let mut out = String::new();

    // Reading -> candidates, in preference order.
    let mut dictionary: HashMap<String, Vec<String>> = HashMap::new();
    // Frequent candidates precede the source dictionary's lexical ordering.
    for (key, values) in [
        ("ni", "你 妳 尼"),
        ("hao", "好 號"),
        ("nihao", "你好"),
        ("zhongguo", "中國"),
        ("wo", "我"),
        ("shi", "是 時 事"),
        ("de", "的 得 地"),
        ("le", "了 乐"),
        ("ma", "吗 嘛 马"),
        ("xiexie", "謝謝"),
        ("zaijian", "再見"),
        ("shijie", "世界"),
        ("zhongwen", "中文"),
    ] {
        dictionary.insert(
            key.into(),
            values.split_whitespace().map(str::to_owned).collect(),
        );
    }
    // This source contains phrase frequencies. Sort before merging: source
    // file order is not candidate preference (rare glyphs often come first).
    let mut weighted: HashMap<String, Vec<(&str, u64)>> = HashMap::new();
    for line in simp.lines().chain(words.lines()) {
        let mut fields = line.split('\t');
        if let (Some(word), Some(reading), Some(weight)) =
            (fields.next(), fields.next(), fields.next())
        {
            let key = reading.replace(' ', "");
            if key.is_empty() || !key.bytes().all(|b| b.is_ascii_lowercase()) {
                continue;
            }
            if let Ok(weight) = weight.parse::<u64>() {
                weighted.entry(key).or_default().push((word, weight));
            }
        }
    }
    for (key, mut entries) in weighted {
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let values = dictionary.entry(key).or_default();
        for (word, _) in entries {
            if !values.iter().any(|v| v == word) {
                values.push(word.to_owned());
            }
        }
    }
    for line in luna.lines() {
        let mut fields = line.split('\t');
        if let (Some(word), Some(reading)) = (fields.next(), fields.next()) {
            let key = reading.replace(' ', "");
            if !key.chars().all(|c| c.is_ascii_lowercase()) {
                continue;
            }
            let values = dictionary.entry(key).or_default();
            if !values.iter().any(|v| v == word) {
                values.push(word.to_owned());
            }
        }
    }
    let mut entries: Vec<_> = dictionary.into_iter().collect();
    entries.sort();
    out.push_str("/// Pinyin reading -> space-separated candidates, sorted by reading.\nstatic DICTIONARY: &[(&str, &str)] = &[\n");
    for (key, values) in &entries {
        writeln!(out, "({key:?},{:?}),", values.join(" ")).unwrap();
    }
    out.push_str("];\n");
    let longest = entries.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    writeln!(out, "/// Length of the longest reading in DICTIONARY.\nconst LONGEST_READING: usize = {longest};").unwrap();

    // Single syllables, for display-only separators.
    let mut syllables: Vec<&str> = [simp.as_str(), luna.as_str()]
        .iter()
        .flat_map(|source| source.lines())
        .filter_map(|line| line.split('\t').nth(1))
        .flat_map(str::split_whitespace)
        .filter(|s| s.bytes().all(|b| b.is_ascii_lowercase()))
        .collect();
    sorted_list(&mut out, "PINYIN_SYLLABLES", &mut syllables);

    let vietnamese = fs::read_to_string("data/vietnamese-syllables.txt").unwrap();
    let keys: Vec<String> = vietnamese.lines().map(spelling::key).collect();
    let mut keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    sorted_list(&mut out, "VIETNAMESE_SYLLABLES", &mut keys);
    // Toneless spellings (đ, ă, â, ê, ô, ơ, ư kept) for prefix checks while typing.
    let bases: Vec<String> = vietnamese.lines().map(spelling::bases).collect();
    let mut bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    sorted_list(&mut out, "VIETNAMESE_BASES", &mut bases);

    let english = fs::read_to_string("data/english-common.txt").unwrap();
    let mut words: Vec<&str> = english.split_whitespace().collect();
    sorted_list(&mut out, "ENGLISH_COMMON", &mut words);

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("tables.rs");
    fs::write(path, out).unwrap();
}

fn sorted_list(out: &mut String, name: &str, items: &mut Vec<&str>) {
    items.sort_unstable();
    items.dedup();
    writeln!(out, "static {name}: &[&str] = &[").unwrap();
    for item in items.iter() {
        writeln!(out, "{item:?},").unwrap();
    }
    out.push_str("];\n");
}
