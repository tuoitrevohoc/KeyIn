//! Offline composition engine. No network, telemetry, or per-keystroke disk access.
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, CStr, CString},
};

pub mod controller;
pub mod meanings;
pub mod rime;

include!("spelling.rs");
include!(concat!(env!("OUT_DIR"), "/tables.rs"));

fn valid_vietnamese(text: &str) -> bool {
    VIETNAMESE_SYLLABLES
        .binary_search(&spelling_key(text).as_str())
        .is_ok()
}

fn english_common(word: &str) -> bool {
    ENGLISH_COMMON.binary_search(&word).is_ok()
}

/// Candidates for an exact Pinyin reading, best first.
fn lookup(reading: &str) -> Option<&'static str> {
    DICTIONARY
        .binary_search_by(|(key, _)| key.cmp(&reading))
        .ok()
        .map(|i| DICTIONARY[i].1)
}

/// Whether any entry of a sorted table starts with `prefix`.
fn has_prefix<'a>(sorted: impl Fn(usize) -> &'a str, len: usize, prefix: &str) -> bool {
    let (mut low, mut high) = (0, len);
    while low < high {
        let mid = (low + high) / 2;
        if sorted(mid) < prefix {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low < len && sorted(low).starts_with(prefix)
}

fn case(c: char, original: char) -> char {
    if original.is_uppercase() {
        c.to_uppercase().next().unwrap()
    } else {
        c
    }
}

pub fn telex(raw: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut tone = 0;
    let mut last_tone = None;
    let mut shortcut_w = false;
    for ch in raw.chars() {
        let previous_shortcut_w = shortcut_w;
        shortcut_w = false;
        let c = ch.to_ascii_lowercase();
        if let Some(t) = "sfrxj".find(c) {
            if out
                .iter()
                .any(|c| VOWELS.iter().any(|s| s.contains(base(*c))))
            {
                if last_tone == Some(c) {
                    tone = 0;
                    last_tone = None;
                    out.push(ch);
                } else {
                    tone = t + 1;
                    last_tone = Some(c);
                }
                continue;
            }
        }
        if c == 'z' && tone != 0 {
            tone = 0;
            last_tone = None;
            continue;
        }
        let replacement = match c {
            'a' => Some(('a', 'â')),
            'e' => Some(('e', 'ê')),
            'o' => Some(('o', 'ô')),
            'd' => Some(('d', 'đ')),
            _ => None,
        };
        if let Some((from, to)) = replacement {
            if let Some(last) = out.last_mut() {
                if base(*last) == from {
                    *last = case(to, *last);
                    continue;
                }
                if base(*last) == to {
                    *last = case(from, *last);
                    out.push(ch);
                    continue;
                }
            }
        }
        if c == 'w' {
            // Escape an inserted shortcut anywhere: nhww -> nhw.
            if previous_shortcut_w {
                let last = out.last_mut().unwrap();
                *last = case('w', *last);
                continue;
            }
            let mut changed = false;
            for value in out.iter_mut().rev().take(3) {
                let to = match base(*value) {
                    'a' => 'ă',
                    'o' => 'ơ',
                    'u' => 'ư',
                    _ => continue,
                };
                *value = case(to, *value);
                changed = true;
                if to == 'ă' {
                    break;
                }
            }
            if changed {
                continue;
            }
            out.push(case('ư', ch));
            shortcut_w = true;
            continue;
        }
        out.push(ch);
    }
    let mut indices: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, c)| VOWELS.iter().any(|s| s.starts_with(base(**c))))
        .map(|(i, _)| i)
        .collect();
    // qu / gi carry an initial glide, except when it is the only vowel.
    if indices.len() > 1
        && out.len() > 1
        && ((base(out[0]) == 'q' && base(out[1]) == 'u')
            || (base(out[0]) == 'g' && base(out[1]) == 'i'))
    {
        indices.retain(|i| *i != 1);
    }
    let target = indices
        .iter()
        .rev()
        .find(|i| "êơ".contains(base(out[**i])))
        .copied()
        .or_else(|| match indices.len() {
            0 => None,
            1 => Some(indices[0]),
            n => {
                let last = *indices.last().unwrap();
                Some(if last + 1 < out.len() {
                    last
                } else if n >= 3 {
                    indices[1]
                } else {
                    indices[0]
                })
            }
        });
    if let Some(i) = target {
        let row = VOWELS.iter().find(|s| s.starts_with(base(out[i]))).unwrap();
        out[i] = case(row.chars().nth(tone).unwrap(), out[i]);
    }
    out.into_iter().collect()
}

/// Exact phrases win; otherwise retain the best nine complete dictionary paths.
/// Apostrophes are hard boundaries, and incomplete suffixes never get dropped.
fn pinyin_candidates(key: &str) -> Vec<String> {
    // Every keystroke asks for the same readings several times (composition,
    // language options, corrections), so remember recent answers.
    thread_local! {
        static RECENT: std::cell::RefCell<HashMap<String, Vec<String>>> = Default::default();
    }
    if let Some(hit) = RECENT.with(|recent| recent.borrow().get(key).cloned()) {
        return hit;
    }
    let result = search_pinyin(key);
    RECENT.with(|recent| {
        let mut recent = recent.borrow_mut();
        if recent.len() >= 256 {
            recent.clear();
        }
        recent.insert(key.to_owned(), result.clone());
    });
    result
}

/// Best (cost, text) conversions of a reading prefix.
type Paths = Vec<(usize, String)>;

fn search_pinyin(key: &str) -> Vec<String> {
    if key.is_empty()
        || key.len() > 128
        || !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'')
        || key.starts_with('\'')
        || key.ends_with('\'')
        || key.contains("''")
    {
        return Vec::new();
    }
    if let Some(exact) = lookup(key) {
        return exact.split(' ').map(str::to_owned).collect();
    }
    // Best paths ending at each position depend only on the letters before it,
    // so the table for the previous keystroke is reused up to the first change.
    // Typing a long reading then costs one column per key instead of all of them.
    thread_local! {
        static LAST: std::cell::RefCell<(String, Vec<Paths>)> = Default::default();
    }
    let mut paths = LAST.with(|last| {
        let (last_key, last_paths) = &mut *last.borrow_mut();
        let common = last_key
            .bytes()
            .zip(key.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let mut paths = std::mem::take(last_paths);
        paths.truncate(if paths.is_empty() { 0 } else { common + 1 });
        paths
    });
    if paths.is_empty() {
        paths.push(vec![(0, String::new())]);
    }
    for end in paths.len()..=key.len() {
        if key.as_bytes()[end - 1] == b'\'' {
            paths.push(paths[end - 1].clone());
            continue;
        }
        let mut next = Vec::new();
        // No reading is longer than LONGEST_READING, so only the last few
        // positions can start a word ending here.
        for start in end.saturating_sub(LONGEST_READING)..end {
            if paths[start].is_empty() {
                continue;
            }
            let Some(words) = lookup(&key[start..end]) else {
                continue;
            };
            for (cost, prefix) in &paths[start] {
                for (rank, word) in words.split(' ').take(9).enumerate() {
                    next.push((cost + 100 + rank, format!("{prefix}{word}")));
                }
            }
        }
        next.sort();
        let mut seen = HashSet::new();
        next.retain(|(_, word)| seen.insert(word.clone()));
        next.truncate(9);
        paths.push(next);
    }
    let result = paths[key.len()]
        .iter()
        .map(|(_, word)| word.clone())
        .collect();
    LAST.with(|last| *last.borrow_mut() = (key.to_owned(), paths));
    result
}

/// Fewest-syllable split of lowercase `key` into Pinyin syllables, where the
/// last piece may be an unfinished syllable. Returns the end of each piece.
fn pinyin_segments(key: &str) -> Option<Vec<usize>> {
    let mut paths: Vec<Option<Vec<usize>>> = vec![None; key.len() + 1];
    paths[key.len()] = Some(Vec::new());
    for start in (0..key.len()).rev() {
        for end in (start + 1..=key.len().min(start + 6)).rev() {
            if PINYIN_SYLLABLES.binary_search(&&key[start..end]).is_err() {
                continue;
            }
            if let Some(suffix) = &paths[end] {
                let mut path = vec![end];
                path.extend(suffix);
                if paths[start]
                    .as_ref()
                    .is_none_or(|old| path.len() < old.len())
                {
                    paths[start] = Some(path);
                }
            }
        }
        if paths[start].is_none()
            && has_prefix(
                |i| PINYIN_SYLLABLES[i],
                PINYIN_SYLLABLES.len(),
                &key[start..],
            )
        {
            paths[start] = Some(vec![key.len()]);
        }
    }
    paths.swap_remove(0)
}

/// Display-only syllable separators; the host retains original keystrokes.
fn pinyin_preedit(raw: &str) -> String {
    if !raw.is_ascii() || raw.len() > 128 {
        return raw.to_owned();
    }
    raw.split('\'')
        .map(|part| {
            let Some(ends) = pinyin_segments(&part.to_ascii_lowercase()) else {
                return part.to_owned();
            };
            let mut start = 0;
            ends.iter()
                .map(|&end| {
                    let syllable = &part[start..end];
                    start = end;
                    syllable
                })
                .collect::<Vec<_>>()
                .join("'")
        })
        .collect::<Vec<_>>()
        .join("'")
}

fn pinyin_initials(raw: &str) -> bool {
    (2..=8).contains(&raw.len()) && raw.bytes().all(|b| b"bpmfdtnlgkhjqxrzcsyw".contains(&b))
}

/// Whether `key` begins some dictionary reading, looking at most six letters ahead.
fn pinyin_prefix(key: &str) -> bool {
    key.len() <= 6 && has_prefix(|i| DICTIONARY[i].0, DICTIONARY.len(), key)
}

/// mode: 0 auto, 1 English, 2 Vietnamese, 3 Chinese.
/// Wire format: language, preedit, then candidate lines. Input is bounded by the host.
pub fn compose(raw: &str, mode: u32) -> String {
    compose_with_context(raw, mode, 0)
}

pub(crate) fn compose_with_context(raw: &str, mode: u32, previous: u32) -> String {
    let key = raw.to_ascii_lowercase();
    let vietnamese = telex(raw);
    let english = [
        "as", "is", "of", "or", "if", "us", "was", "has", "his", "her", "for", "our", "your",
        "this", "that", "with", "test", "text", "hello", "world", "office", "coffee", "book",
        "look", "good", "soon", "see", "new", "how", "now", "show", "know", "low", "row", "law",
        "draw", "raw", "saw", "few",
    ];
    // Capitals mark names and English words; automatic detection never reads
    // them as Pinyin (Chinese can still be chosen explicitly).
    let capital = raw.bytes().any(|b| b.is_ascii_uppercase());
    let candidates = if mode == 3 || (mode == 0 && !capital) {
        pinyin_candidates(&key)
    } else {
        Vec::new()
    };
    let detected = if mode != 0 {
        mode
    } else if previous == 2 && valid_vietnamese(&vietnamese) {
        // Resolve valid Vietnamese/English collisions using the typing context.
        2
    } else if previous == 3
        && key.len() <= 3
        && !candidates.is_empty()
        && !(vietnamese != raw && valid_vietnamese(&vietnamese))
    {
        // Short Pinyin particles/pronouns can also be English words (a, me, he).
        // Continue the most recently confirmed Chinese context in this ambiguity.
        3
    } else if english.contains(&key.as_str()) || english_common(&key) {
        1
    } else if vietnamese != raw && valid_vietnamese(&vietnamese) {
        2
    } else if previous == 3
        && !raw.is_empty()
        && !capital
        && (!candidates.is_empty() || pinyin_prefix(&key) || pinyin_initials(raw))
    {
        3
    } else if valid_vietnamese(&vietnamese) {
        2
    } else if !candidates.is_empty() && key.len() >= 4 {
        3
    } else {
        1
    };
    match detected {
        2 => format!("Vietnamese\n{vietnamese}"),
        3 => format!(
            "Chinese\n{}{}",
            pinyin_preedit(raw),
            if candidates.is_empty() {
                String::new()
            } else {
                format!("\n{}", candidates.join("\n"))
            }
        ),
        _ => format!("English\n{raw}"),
    }
}

/// Conservative boundary correction: known English typos and a unique adjacent
/// key transposition validated against Vietnamese / Pinyin dictionaries.
pub(crate) fn correction(raw: &str, mode: u32) -> Option<String> {
    if !(3..=32).contains(&raw.len()) || !raw.is_ascii() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    if mode <= 1 {
        let replacement = match lower.as_str() {
            "teh" => Some("the"),
            "hte" => Some("the"),
            "adn" => Some("and"),
            "recieve" => Some("receive"),
            "helllo" => Some("hello"),
            "becuase" => Some("because"),
            "thier" => Some("their"),
            "woudl" => Some("would"),
            "dont" => Some("don't"),
            _ => None,
        };
        if let Some(word) = replacement {
            let word = if raw.chars().all(|c| c.is_ascii_uppercase()) {
                word.to_uppercase()
            } else if raw.starts_with(|c: char| c.is_ascii_uppercase()) {
                format!("{}{}", word[..1].to_uppercase(), &word[1..])
            } else {
                word.to_owned()
            };
            return Some(format!("English\n{word}"));
        }
    }
    if mode == 1
        || valid_vietnamese(&telex(raw))
        || !pinyin_candidates(&lower).is_empty()
        || english_common(&lower)
    {
        return None;
    }
    // Pinyin still being typed ("...zh") is not a typo; searching every
    // transposition of a long reading was the slowest part of a keystroke.
    let capital = raw.bytes().any(|b| b.is_ascii_uppercase());
    let chinese = (mode == 3 || (mode == 0 && raw.len() >= 6 && !capital))
        && pinyin_segments(&lower).is_none();
    let mut results = HashSet::new();
    for i in 0..raw.len() - 1 {
        let mut bytes = raw.as_bytes().to_vec();
        bytes.swap(i, i + 1);
        let edited = String::from_utf8(bytes).unwrap();
        let vi = telex(&edited);
        if (mode == 0 || mode == 2) && vi != edited && valid_vietnamese(&vi) {
            results.insert(format!("Vietnamese\n{vi}"));
        }
        if chinese {
            let candidates = pinyin_candidates(&edited.to_ascii_lowercase());
            if let Some(first) = candidates.first() {
                results.insert(format!("Chinese\n{raw}\n{first}"));
            }
        }
    }
    if results.is_empty() && chinese && raw.len() <= 16 {
        // A finger landing on the key next to the intended one ("zhomgguo").
        // Prefer edits that are whole dictionary phrases; give up if ambiguous.
        // Longer input is left alone to keep every keystroke cheap.
        let mut phrases = HashSet::new();
        let mut paths = HashSet::new();
        for (i, c) in lower.bytes().enumerate() {
            for &near in neighbors(c) {
                let mut bytes = lower.as_bytes().to_vec();
                bytes[i] = near;
                let edited = String::from_utf8(bytes).unwrap();
                if let Some(words) = lookup(&edited) {
                    phrases.insert(words.split(' ').next().unwrap().to_owned());
                } else if let Some(first) = pinyin_candidates(&edited).first() {
                    paths.insert(first.clone());
                }
            }
        }
        let best = if phrases.is_empty() { paths } else { phrases };
        if best.len() == 1 {
            let first = best.into_iter().next().unwrap();
            results.insert(format!("Chinese\n{raw}\n{first}"));
        }
    }
    if results.len() == 1 {
        results.into_iter().next()
    } else {
        None
    }
}

/// Letters physically next to `c` on a QWERTY keyboard.
fn neighbors(c: u8) -> &'static [u8] {
    match c {
        b'q' => b"wa",
        b'w' => b"qeas",
        b'e' => b"wrsd",
        b'r' => b"etdf",
        b't' => b"ryfg",
        b'y' => b"tugh",
        b'u' => b"yihj",
        b'i' => b"uojk",
        b'o' => b"ipkl",
        b'p' => b"ol",
        b'a' => b"qwsz",
        b's' => b"weadzx",
        b'd' => b"ersfxc",
        b'f' => b"rtdgcv",
        b'g' => b"tyfhvb",
        b'h' => b"yugjbn",
        b'j' => b"uihknm",
        b'k' => b"iojlm",
        b'l' => b"opk",
        b'z' => b"asx",
        b'x' => b"zsdc",
        b'c' => b"xdfv",
        b'v' => b"cfgb",
        b'b' => b"vghn",
        b'n' => b"bhjm",
        b'm' => b"njk",
        _ => b"",
    }
}

/// Letters that can sensibly follow `raw`: they continue a Pinyin reading,
/// a Vietnamese syllable (including Telex tone and vowel keys) or a common
/// English word. Bit 0 is 'a'. Hosts use it to enlarge likely keys.
/// Called on every keystroke, so it looks only at the end of the input.
pub fn next_letters(raw: &str) -> u32 {
    if raw.is_empty() || !raw.is_ascii() || raw.len() > 64 {
        return (1 << 26) - 1;
    }
    let lower = raw.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    // reach[i]: lower[..i] splits into whole Pinyin syllables. A letter fits
    // if it extends a syllable started at some reachable position.
    let mut reach = vec![false; bytes.len() + 1];
    reach[0] = true;
    for end in 1..=bytes.len() {
        reach[end] = if bytes[end - 1] == b'\'' {
            reach[end - 1]
        } else {
            (end.saturating_sub(6)..end)
                .any(|i| reach[i] && PINYIN_SYLLABLES.binary_search(&&lower[i..end]).is_ok())
        };
    }
    let starts: Vec<usize> = (bytes.len().saturating_sub(5)..=bytes.len())
        .filter(|&i| reach[i] && !lower[i..].contains('\''))
        .collect();
    // Vietnamese syllables are short; past that only Pinyin can continue.
    let vietnamese = (raw.len() <= 10).then(|| telex(raw));
    let has_vowel = vietnamese
        .as_deref()
        .is_some_and(|v| v.chars().any(|c| VOWELS.iter().any(|row| row.contains(c))));
    let mut mask = 0;
    for (bit, c) in ('a'..='z').enumerate() {
        let typed = format!("{lower}{c}");
        let plausible = starts
            .iter()
            .any(|&i| has_prefix(|j| PINYIN_SYLLABLES[j], PINYIN_SYLLABLES.len(), &typed[i..]))
            || (has_vowel && "sfrxjzw".contains(c))
            || (vietnamese.is_some() && {
                let bases: String = telex(&typed).chars().map(base).collect();
                has_prefix(|i| VIETNAMESE_BASES[i], VIETNAMESE_BASES.len(), &bases)
            })
            || has_prefix(|i| ENGLISH_COMMON[i], ENGLISH_COMMON.len(), &typed);
        if plausible {
            mask |= 1 << bit;
        }
    }
    mask
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_next_letters(raw: *const c_char) -> u32 {
    if raw.is_null() {
        return (1 << 26) - 1;
    }
    next_letters(&unsafe { CStr::from_ptr(raw) }.to_string_lossy())
}

/// # Safety
/// `raw` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_correct(raw: *const c_char, mode: u32) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(correction(&raw, mode).unwrap_or_default())
        .unwrap()
        .into_raw()
}

/// Languages available for explicit selection: bits 1 English, 2 Vietnamese,
/// 3 Chinese. Literal input is always available; manual modes remain unrestricted.
pub(crate) fn language_options(raw: &str) -> u32 {
    if raw.is_empty() {
        return 0;
    }
    let mut options = 1 << 1;
    if valid_vietnamese(&telex(raw)) {
        options |= 1 << 2;
    }
    if !pinyin_candidates(&raw.to_ascii_lowercase()).is_empty() || pinyin_initials(raw) {
        options |= 1 << 3;
    }
    options
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_language_options(raw: *const c_char) -> u32 {
    if raw.is_null() {
        return 0;
    }
    language_options(&unsafe { CStr::from_ptr(raw) }.to_string_lossy())
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_compose_context(
    raw: *const c_char,
    mode: u32,
    previous: u32,
) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(compose_with_context(&raw, mode, previous))
        .unwrap()
        .into_raw()
}

/// Everything a host needs for one keystroke in a single call: language
/// options, the correction (`correct_mode` above 3 skips it), then the
/// composition, separated by U+001E. Lookups shared by the three are computed once.
pub fn analyze(raw: &str, mode: u32, previous: u32, correct_mode: u32) -> String {
    let correction = if correct_mode <= 3 {
        correction(raw, correct_mode)
    } else {
        None
    };
    format!(
        "{}\u{1e}{}\u{1e}{}",
        language_options(raw),
        correction.unwrap_or_default(),
        compose_with_context(raw, mode, previous)
    )
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn keyin_analyze(
    raw: *const c_char,
    mode: u32,
    previous: u32,
    correct_mode: u32,
) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(analyze(&raw, mode, previous, correct_mode))
        .unwrap()
        .into_raw()
}

/// # Safety
/// `raw` must point to a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
pub unsafe extern "C" fn keyin_compose(raw: *const c_char, mode: u32) -> *mut c_char {
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { CStr::from_ptr(raw) }.to_string_lossy();
    CString::new(compose(&raw, mode)).unwrap().into_raw()
}
/// # Safety
/// `ptr` must be an unfreed pointer returned by `keyin_compose`.
#[no_mangle]
pub unsafe extern "C" fn keyin_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vietnamese_words() {
        for (input, expected) in [
            ("tieengs", "tiếng"),
            ("Vieetj", "Việt"),
            ("ddawng", "đăng"),
            ("dduwowngf", "đường"),
            ("nguwowif", "người"),
            ("hoaf", "hòa"),
            ("quas", "quá"),
            ("gif", "gì"),
            ("ass", "as"),
            ("aaa", "aa"),
            ("asz", "a"),
        ] {
            assert_eq!(telex(input), expected, "{input}");
        }
    }
    #[test]
    fn detection_and_overrides() {
        assert_eq!(compose("hello", 0), "English\nhello");
        assert_eq!(compose("tieengs", 0), "Vietnamese\ntiếng");
        assert!(compose("nihao", 0).starts_with("Chinese\nni'hao\n你好"));
        assert_eq!(compose("nihao", 1), "English\nnihao");
        assert!(compose("ni", 3).contains("\n你"));
        assert_eq!(compose("as", 2), "Vietnamese\ná");
    }
    #[test]
    fn editing_recomputes_from_original_keystrokes() {
        assert_eq!(telex("tieengs"), "tiếng");
        assert_eq!(telex("tieeng"), "tiêng");
        assert_eq!(telex("tiee"), "tiê");
        assert_eq!(telex(""), "");
        assert_eq!(telex("DDAS"), "ĐÁ");
        assert_eq!(compose("rust", 0), "English\nrust");
        assert_eq!(compose("ni'hao", 3).lines().nth(2), Some("你好"));
    }
    #[test]
    fn arbitrary_unicode_and_unknown_readings_are_preserved() {
        assert_eq!(compose("🙂", 1), "English\n🙂");
        assert_eq!(compose("zzzzzz", 3), "Chinese\nzzzzzz");
        for text in [
            "", "a", "s", "w", "z", "đ", "中文", "aáà", "uwow", "ssss", "aaaa",
        ] {
            for mode in 0..4 {
                let _ = compose(text, mode);
            }
        }
    }
    #[test]
    fn analyze_combines_the_three_calls() {
        let parts: Vec<String> = analyze("tieengs", 0, 0, 0)
            .split('\u{1e}')
            .map(String::from)
            .collect();
        assert_eq!(parts[0], language_options("tieengs").to_string());
        assert_eq!(parts[1], correction("tieengs", 0).unwrap_or_default());
        assert_eq!(parts[2], compose_with_context("tieengs", 0, 0));
        assert_eq!(analyze("teh", 0, 0, 9).split('\u{1e}').nth(1), Some(""));
    }
    #[test]
    fn incremental_pinyin_matches_a_fresh_search() {
        let sentence = "woxiangqubeijingkankantiananmenguangchang";
        for end in 1..=sentence.len() {
            let _ = pinyin_candidates(&sentence[..end]);
        }
        let fresh = std::thread::spawn(move || search_pinyin(sentence))
            .join()
            .unwrap();
        assert_eq!(search_pinyin(sentence), fresh);
        // Editing in the middle reuses only the unchanged prefix.
        let edited = "woxiangqushanghai";
        let fresh = std::thread::spawn(move || search_pinyin(edited))
            .join()
            .unwrap();
        assert_eq!(search_pinyin(edited), fresh);
    }
    #[test]
    fn neighbouring_key_typos_in_pinyin() {
        assert_eq!(correction("zhongguo", 3), None);
        assert_eq!(
            correction("zhomgguo", 3),
            Some("Chinese\nzhomgguo\n中國".into())
        );
    }
    #[test]
    fn likely_next_letters() {
        let letters = |raw: &str| {
            let mask = next_letters(raw);
            ('a'..='z')
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, c)| c)
                .collect::<String>()
        };
        assert!(letters("zh").contains('a') && !letters("zh").contains('q'));
        assert!(letters("q").contains('u') && !letters("q").contains('b'));
        assert!(letters("tie").contains('e') && letters("tie").contains('s'));
        assert_eq!(next_letters("").count_ones(), 26);
    }
    #[test]
    fn capitals_are_not_detected_as_pinyin() {
        assert!(compose("nihao", 0).starts_with("Chinese"));
        assert!(!compose("Nihao", 0).starts_with("Chinese"));
        assert!(!compose_with_context("Wo", 0, 3).starts_with("Chinese"));
        assert!(compose("Nihao", 3).starts_with("Chinese\nNi'hao\n你好"));
    }
    #[test]
    fn dictionary_coverage() {
        assert!(DICTIONARY.len() > 10000);
        assert!(VIETNAMESE_SYLLABLES.len() > 6000);
    }
    #[test]
    fn automatic_pinyin_separators_are_display_only() {
        for (raw, expected) in [
            ("nihaolema", "ni'hao'le'ma"),
            ("zenmeyang", "zen'me'yang"),
            ("kankan", "kan'kan"),
            ("NIHAO", "NI'HAO"),
            ("ni'hao", "ni'hao"),
            ("xi'an", "xi'an"),
            ("xian", "xian"),
            ("nihaol", "ni'hao'l"),
            ("nihao'", "ni'hao'"),
            ("zzzzzz", "zzzzzz"),
        ] {
            assert_eq!(pinyin_preedit(raw), expected, "{raw}");
            assert_eq!(compose(raw, 3).lines().nth(1), Some(expected), "{raw}");
        }
        assert_eq!(compose("nihao", 1), "English\nnihao");
        assert_eq!(compose("nhwng", 0), "Vietnamese\nnhưng");
    }
    #[test]
    fn frequent_simplified_phrases_rank_first() {
        for (raw, expected) in [
            ("kankan", "看看"),
            ("zenmeyang", "怎么样"),
            ("kan", "看"),
            ("zenme", "怎么"),
            ("kan'kan", "看看"),
            ("zen'me'yang", "怎么样"),
        ] {
            assert_eq!(
                pinyin_candidates(raw).first().map(String::as_str),
                Some(expected),
                "{raw}"
            );
            assert_eq!(compose(raw, 3).lines().nth(2), Some(expected), "{raw}");
            if raw.len() >= 4 {
                assert_eq!(compose(raw, 0).lines().nth(2), Some(expected), "{raw}");
            }
        }
        assert_eq!(compose("kankan", 1), "English\nkankan");
    }
    #[test]
    fn standalone_w_and_escape() {
        for (raw, expected) in [
            ("w", "ư"),
            ("W", "Ư"),
            ("ws", "ứ"),
            ("ww", "w"),
            ("WW", "W"),
            ("uw", "ư"),
            ("nhwng", "nhưng"),
            ("NHWNG", "NHƯNG"),
            ("nhwngz", "nhưngz"),
            ("nhww", "nhw"),
        ] {
            assert_eq!(telex(raw), expected);
        }
        assert_eq!(compose("w", 0), "Vietnamese\nư");
        assert_eq!(compose("nhwng", 0), "Vietnamese\nnhưng");
        assert_eq!(compose("nhwng", 2), "Vietnamese\nnhưng");
        assert_eq!(compose("nhwng", 1), "English\nnhwng");
        assert_eq!(compose("world", 0), "English\nworld");
        assert!(compose("wo", 3).starts_with("Chinese\nwo\n我"));
    }
    #[test]
    fn pinyin_phrases_are_segmented_without_losing_suffixes() {
        for raw in ["nihaolema", "ni'hao'le'ma", "NIHAOLEMA"] {
            // Capitals are only read as Pinyin when Chinese is chosen explicitly.
            let modes: &[u32] = if raw == "NIHAOLEMA" { &[3] } else { &[0, 3] };
            for &mode in modes {
                assert_eq!(compose(raw, mode).lines().nth(2), Some("你好了吗"), "{raw}");
            }
        }
        assert_eq!(compose("nihaolema", 1), "English\nnihaolema");
        assert!(pinyin_candidates("nihaolemaq").is_empty());
        assert!(pinyin_candidates("ni''hao").is_empty());
        assert!(pinyin_candidates("🙂").is_empty());
        assert!(pinyin_candidates(&"a".repeat(129)).is_empty());
        assert_eq!(pinyin_candidates("nihaole")[0], "你好了");
        assert_eq!(pinyin_candidates("nihao")[0], "你好");
        assert!(!pinyin_candidates("xi'an").contains(&"先".to_string()));
    }
    #[test]
    fn vietnamese_dictionary_resolves_pinyin_collisions() {
        for (raw, expected) in [
            ("chao", "chao"),
            ("chong", "chong"),
            ("hoaf", "hòa"),
            ("thuyr", "thủy"),
            ("Vieetj", "Việt"),
        ] {
            assert_eq!(compose(raw, 0), format!("Vietnamese\n{expected}"));
        }
        for raw in ["nihao", "zhongguo", "xiexie", "shijie", "zhongwen"] {
            assert!(compose(raw, 0).starts_with("Chinese\n"), "{raw}");
        }
        assert_eq!(compose("qzuw", 0), "English\nqzuw");
        assert_eq!(compose("qzuw", 2), "Vietnamese\nqzư");
        assert!(compose("chong", 3).starts_with("Chinese\nchong\n"));
    }
    #[test]
    fn contextual_detection_can_switch_without_getting_stuck() {
        for raw in ["ne", "ma", "ba", "me", "he", "a", "n"] {
            assert!(
                compose_with_context(raw, 0, 3).starts_with("Chinese\n"),
                "{raw}"
            );
        }
        assert_eq!(compose_with_context("me", 0, 1), "English\nme");
        assert_eq!(compose_with_context("ne", 1, 3), "English\nne");
        assert!(compose_with_context("ni", 0, 3).starts_with("Chinese\n"));
        assert!(compose_with_context("hao", 0, 3).starts_with("Chinese\n"));
        assert!(compose_with_context("zh", 0, 3).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("hello", 0, 3), "English\nhello");
        assert_eq!(compose_with_context("nhwng", 0, 3), "Vietnamese\nnhưng");
        assert_eq!(compose_with_context("tieengs", 0, 3), "Vietnamese\ntiếng");
        assert!(compose_with_context("zenmeyang", 0, 2).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("chao", 0, 2), "Vietnamese\nchao");
        assert!(compose_with_context("chao", 0, 3).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("ni", 1, 3), "English\nni");
        assert_eq!(compose_with_context("nhwng", 2, 3), "Vietnamese\nnhưng");
    }
    #[test]
    fn vietnamese_context_wins_ambiguous_english_words() {
        assert_eq!(compose_with_context("its", 0, 2), "Vietnamese\nít");
        assert_eq!(compose_with_context("its", 0, 1), "English\nits");
        assert_eq!(compose_with_context("its", 1, 2), "English\nits");
        assert_eq!(compose_with_context("hello", 0, 2), "English\nhello");
        assert_eq!(compose_with_context("as", 0, 2), "Vietnamese\ná");
    }

    #[test]
    fn initials_need_chinese_context_and_preserve_telex() {
        assert!(compose_with_context("nhlm", 0, 3).starts_with("Chinese\n"));
        assert!(compose_with_context("zmy", 0, 3).starts_with("Chinese\n"));
        for previous in [0, 1, 2] {
            assert_eq!(compose_with_context("nhlm", 0, previous), "English\nnhlm");
        }
        assert_eq!(compose_with_context("nhwng", 0, 3), "Vietnamese\nnhưng");
        assert_eq!(compose_with_context("hello", 0, 3), "English\nhello");
        assert_eq!(
            compose_with_context("HTTP", 0, 3).lines().nth(1),
            Some("HTTP")
        );
        assert!(!compose_with_context("HTTP", 0, 3).starts_with("Chinese\n"));
        assert_eq!(compose_with_context("nhlm", 1, 3), "English\nnhlm");
    }
    #[test]
    fn explicit_language_alternatives() {
        assert_eq!(language_options(""), 0);
        assert_eq!(language_options("chao"), (1 << 1) | (1 << 2) | (1 << 3));
        assert_eq!(language_options("nhlm"), (1 << 1) | (1 << 3));
        assert_eq!(language_options("🙂"), 1 << 1);
        assert!(language_options("nhwng") & (1 << 2) != 0);
    }
    #[test]
    fn ffi_ownership() {
        unsafe {
            let input = CString::new("tieengs").unwrap();
            let result = keyin_compose(input.as_ptr(), 2);
            assert_eq!(
                CStr::from_ptr(result).to_str().unwrap(),
                "Vietnamese\ntiếng"
            );
            keyin_free(result);
            keyin_free(std::ptr::null_mut());
        }
    }
    #[test]
    fn boundary_typo_correction() {
        assert_eq!(correction("teh", 0), Some("English\nthe".into()));
        assert_eq!(correction("Recieve", 1), Some("English\nReceive".into()));
        assert_eq!(correction("DONT", 1), Some("English\nDON'T".into()));
        for raw in ["nhwng", "hello", "chong", "nihao", "🙂", ""] {
            assert_eq!(correction(raw, 0), None, "{raw}");
        }
        assert_eq!(correction("teh", 3), None);
        assert_eq!(correction("tienegs", 2), Some("Vietnamese\ntiếng".into()));
        assert_eq!(correction("nhwgn", 2), Some("Vietnamese\nnhưng".into()));
        assert_eq!(correction("tienegs", 1), None);
        assert_eq!(correction("nihaolmea", 3), None); // Ambiguous: do not guess.
    }
}
