//! Platform-neutral input controller: key handling, language switching, candidate
//! selection and commit logic. Hosts (TSF on Windows) translate their key events into
//! [`KeyEvent`], apply [`Output`] to the document and draw [`Controller::panel`].
//! Mirrors the macOS InputMethodKit controller in native/main.m.
use crate::{meanings::Meanings, rime::Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English = 1,
    Vietnamese = 2,
    Chinese = 3,
}

impl Language {
    pub fn from_index(index: u32) -> Language {
        match index {
            2 => Language::Vietnamese,
            3 => Language::Chinese,
            _ => Language::English,
        }
    }

    fn from_name(name: &str) -> Option<Language> {
        match name {
            "English" => Some(Language::English),
            "Vietnamese" => Some(Language::Vietnamese),
            "Chinese" => Some(Language::Chinese),
            _ => None,
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Language::English => "EN",
            Language::Vietnamese => "VI",
            Language::Chinese => "简中",
        }
    }
}

/// User preferences; the host loads and persists them (registry, NSUserDefaults).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub automatic: bool,
    /// Manual language, 1 English, 2 Vietnamese, 3 Chinese.
    pub language: u32,
    pub autocorrect: bool,
    pub learn_chinese: bool,
    /// 0 off, 1 Vietnamese, 2 English.
    pub meaning_language: u32,
    /// Bits 1–3 (EN, VI, 中); at least one always stays enabled.
    pub enabled_languages: u32,
    /// Tab switches language / Shift+Tab toggles Auto while composing.
    pub tab_shortcuts: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            automatic: true,
            language: 1,
            autocorrect: true,
            learn_chinese: true,
            meaning_language: 0,
            enabled_languages: 14,
            tab_shortcuts: true,
        }
    }
}

impl Settings {
    pub fn enabled(&self) -> u32 {
        match self.enabled_languages & 14 {
            0 => 14,
            mask => mask,
        }
    }

    /// 0 auto, otherwise the pinned language. A single enabled language behaves like manual mode.
    pub fn mode(&self) -> u32 {
        let enabled = self.enabled();
        if enabled & (enabled - 1) == 0 {
            return first_language(enabled);
        }
        if self.automatic {
            return 0;
        }
        if enabled & (1 << self.language) != 0 {
            self.language
        } else {
            first_language(enabled)
        }
    }
}

fn first_language(mask: u32) -> u32 {
    (1..=3)
        .find(|language| mask & (1 << language) != 0)
        .unwrap_or(1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Escape,
    Space,
    Return,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    /// Any other key that produces input or moves the caret; commits the composition.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub shift: bool,
    /// Control, Alt/Option or Command is held: a shortcut, never composition input.
    pub shortcut: bool,
}

impl KeyEvent {
    pub fn new(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            shift: false,
            shortcut: false,
        }
    }
}

/// What the host must apply to the document after a call, in this order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// Text to insert, replacing any composition.
    pub commit: String,
    /// The composition (preedit + caret) changed and must be redrawn.
    pub composition_changed: bool,
}

pub struct PanelItem {
    pub number: usize,
    pub text: String,
    pub meaning: Option<String>,
}

/// Candidate window content; the host only does layout and drawing.
pub struct Panel {
    pub language: Language,
    pub hints: Vec<String>,
    pub items: Vec<PanelItem>,
    pub page: usize,
    pub pages: usize,
    /// Index in `items` of the highlighted candidate.
    pub highlight: usize,
    pub show_meanings: bool,
}

pub struct Controller {
    pub settings: Settings,
    raw: String,
    preedit: String,
    language: Language,
    choices: Vec<String>,
    page: usize,
    /// Highlighted candidate on the current page; ↑↓ move it, Space / Return commit it.
    highlight: usize,
    correction: Option<String>,
    correction_language: Option<Language>,
    rime: Option<Session>,
    using_rime: bool,
    rime_editing: bool,
    recent: Vec<Language>,
    caret: usize,
    word_mode: u32,
    language_options: Vec<u32>,
    simplify: Box<dyn Fn(&str) -> String>,
    output: Output,
    settings_changed: bool,
}

impl Controller {
    /// `rime` is `None` when librime is unavailable; Pinyin then uses the built-in fallback.
    /// `simplify` converts fallback Traditional candidates to Simplified Chinese.
    pub fn new(
        settings: Settings,
        rime: Option<Session>,
        simplify: Box<dyn Fn(&str) -> String>,
    ) -> Controller {
        Controller {
            settings,
            raw: String::new(),
            preedit: String::new(),
            language: Language::English,
            choices: Vec::new(),
            page: 0,
            highlight: 0,
            correction: None,
            correction_language: None,
            rime,
            using_rime: false,
            rime_editing: false,
            recent: Vec::new(),
            caret: 0,
            word_mode: 0,
            language_options: Vec::new(),
            simplify,
            output: Output::default(),
            settings_changed: false,
        }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn is_composing(&self) -> bool {
        !self.raw.is_empty()
    }

    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Caret inside the preedit, in UTF-16 code units.
    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn choices(&self) -> &[String] {
        &self.choices
    }

    pub fn using_rime(&self) -> bool {
        self.using_rime
    }

    pub fn word_mode(&self) -> u32 {
        self.word_mode
    }

    pub fn language_options(&self) -> &[u32] {
        &self.language_options
    }

    pub fn recent_languages(&self) -> &[Language] {
        &self.recent
    }

    pub fn rime_available(&self) -> bool {
        self.rime.is_some()
    }

    pub fn take_output(&mut self) -> Output {
        std::mem::take(&mut self.output)
    }

    /// True once after a call changed settings that the host must persist.
    pub fn take_settings_changed(&mut self) -> bool {
        std::mem::take(&mut self.settings_changed)
    }

    pub fn activate(&mut self) {
        let mode = self.settings.mode();
        if mode != 0 {
            self.language = Language::from_index(mode);
        }
    }

    pub fn deactivate(&mut self) {
        self.commit_composition();
        self.recent.clear();
        if self.settings.mode() == 0 {
            self.language = Language::English;
        }
    }

    /// Focus changes preserve unselected Pinyin instead of silently choosing a character.
    pub fn commit_composition(&mut self) {
        let text = if self.language == Language::Chinese {
            self.raw.clone()
        } else {
            self.preedit.clone()
        };
        self.finish(text);
    }

    /// Drops the composition without inserting anything; the host keeps the text it shows.
    pub fn reset(&mut self) {
        self.raw.clear();
        self.finish(String::new());
        self.output = Output::default();
    }

    /// Returns true when the key was consumed; otherwise the application receives it.
    pub fn handle_key(&mut self, event: KeyEvent) -> bool {
        if event.shortcut {
            self.commit_composition();
            return false;
        }
        // Only claim Tab while a word is being composed; otherwise apps keep their own Tab.
        if event.key == Key::Tab && !self.raw.is_empty() {
            if !self.settings.tab_shortcuts {
                self.commit_composition();
                return false;
            }
            if event.shift {
                self.toggle_automatic_while_typing();
            } else {
                self.switch_language();
            }
            return true;
        }
        if !event.shift && matches!(event.key, Key::Up | Key::Down) {
            let count = self.choices.len();
            if count == 0 {
                self.commit_composition();
                return false;
            }
            let step = if event.key == Key::Down { 1 } else { count - 1 };
            let next = (self.highlighted_index() + step) % count;
            self.set_page(next / 9);
            self.highlight = next % 9;
            return true;
        }
        if self.using_rime
            && !self.raw.is_empty()
            && (matches!(event.key, Key::Home | Key::End)
                || (event.shift && matches!(event.key, Key::Left | Key::Right)))
        {
            let key = match event.key {
                Key::Home => 0xff50,
                Key::End => 0xff57,
                Key::Left => 0xff51,
                _ => 0xff53,
            };
            if let Some(rime) = &mut self.rime {
                rime.process_key(key);
            }
            self.rime_editing = true;
            self.set_page(0);
            self.render_rime();
            return true;
        }
        if event.key == Key::Backspace && !self.raw.is_empty() {
            if self.using_rime {
                if let Some(rime) = &mut self.rime {
                    let caret = rime.caret();
                    rime.process_key(0xff08);
                    let updated = rime.input();
                    if self.raw.to_lowercase() != updated {
                        let mut edited = self.raw.clone();
                        if caret > 0 && caret <= edited.len() {
                            edited.remove(caret - 1);
                        }
                        self.raw = if edited.to_lowercase() == updated {
                            edited
                        } else {
                            updated
                        };
                    }
                }
                self.set_page(0);
                self.render_rime();
                return true;
            }
            self.raw.pop();
            self.set_page(0);
            self.refresh();
            return true;
        }
        if event.key == Key::Escape && !self.raw.is_empty() {
            self.finish(self.raw.clone());
            return true;
        }
        if !self.choices.is_empty() && matches!(event.key, Key::Left | Key::Right) {
            let pages = self.choices.len().div_ceil(9);
            self.set_page(
                (self.page
                    + if event.key == Key::Right {
                        1
                    } else {
                        pages - 1
                    })
                    % pages,
            );
            return true;
        }
        if let Key::Char(c @ '1'..='9') = event.key {
            if !self.choices.is_empty() {
                let index = self.page * 9 + (c as usize - '1' as usize);
                if index < self.choices.len() {
                    if self.using_rime {
                        self.select_rime_candidate(index);
                    } else {
                        self.remember_language();
                        self.finish(self.choices[index].clone());
                    }
                }
                return true;
            }
        }
        if matches!(event.key, Key::Space | Key::Return) {
            if self.raw.is_empty() {
                return false;
            }
            if self.using_rime && !self.choices.is_empty() {
                self.select_rime_candidate(self.highlighted_index());
                return true;
            }
            let chinese =
                !self.choices.is_empty() || self.correction_language == Some(Language::Chinese);
            let value = match &self.correction {
                Some(correction) => correction.clone(),
                None if chinese => self.choices[self.highlighted_index()].clone(),
                None if self.language == Language::Chinese => self.raw.clone(),
                None => self.preedit.clone(),
            };
            if let Some(language) = self.correction_language {
                self.language = language;
            }
            self.remember_language();
            self.finish(value);
            return chinese;
        }
        match event.key {
            Key::Char(c) if c.is_ascii_alphabetic() || (c == '\'' && !self.raw.is_empty()) => {
                if self.raw.len() >= 128 {
                    self.commit_composition();
                }
                if self.using_rime && self.rime_editing {
                    if let Some(rime) = &mut self.rime {
                        let mut edited = self.raw.clone();
                        edited.insert(rime.caret().min(edited.len()), c);
                        if rime.process_key(c.to_ascii_lowercase() as i32) {
                            let updated = rime.input();
                            self.raw = if edited.to_lowercase() == updated {
                                edited
                            } else {
                                updated
                            };
                            self.set_page(0);
                            self.render_rime();
                            return true;
                        }
                    }
                }
                self.raw.push(c);
                self.set_page(0);
                self.refresh();
                true
            }
            _ => {
                self.commit_composition();
                false
            }
        }
    }

    pub fn set_settings(&mut self, settings: Settings) {
        if settings != self.settings {
            self.settings = settings;
            if self.is_composing() {
                self.refresh();
            }
        }
    }

    pub fn panel(&self, meanings: Option<&Meanings>) -> Panel {
        let pages = self.choices.len().div_ceil(9);
        let start = self.page * 9;
        let language_key = match self.settings.meaning_language {
            1 => Some("vi"),
            2 => Some("en"),
            _ => None,
        };
        let show_meanings = !self.choices.is_empty()
            && self.language == Language::Chinese
            && language_key.is_some()
            && meanings.is_some();
        let items = self
            .choices
            .iter()
            .skip(start)
            .take(9)
            .enumerate()
            .map(|(i, text)| PanelItem {
                number: i + 1,
                text: text.clone(),
                meaning: if show_meanings {
                    meanings
                        .zip(language_key)
                        .and_then(|(m, key)| m.annotate(text, key))
                } else {
                    None
                },
            })
            .collect();
        let mode = self.settings.mode();
        let mut hints = vec![if self.word_mode != 0 {
            "Chọn"
        } else if mode != 0 {
            "Manual"
        } else {
            "Auto"
        }
        .to_owned()];
        if let Some(correction) = &self.correction {
            hints.push(format!("Space → {correction}"));
            hints.push("Esc giữ gốc".to_owned());
        } else if !self.choices.is_empty() {
            if pages > 1 {
                hints.push(format!("{}/{pages}", self.page + 1));
            }
            hints.push("↑↓ 1–9 chọn".to_owned());
            hints.push("Space nhập".to_owned());
            if pages > 1 {
                hints.push("← → trang".to_owned());
            }
        }
        if !self.raw.is_empty() && self.settings.tab_shortcuts {
            hints.push("Tab đổi tiếng".to_owned());
            hints.push("⇧Tab Auto".to_owned());
        }
        Panel {
            language: self.correction_language.unwrap_or(self.language),
            hints,
            items,
            page: self.page,
            pages,
            highlight: self.highlight,
            show_meanings,
        }
    }

    fn context_language(&self) -> u32 {
        match self.language {
            Language::Chinese if !self.raw.is_empty() => 3,
            Language::Vietnamese if !self.raw.is_empty() => 2,
            // Retain confirmed context through temporary ambiguous prefixes.
            _ => self.recent.last().map_or(0, |&language| language as u32),
        }
    }

    fn remember_language(&mut self) {
        self.recent.push(self.language);
        if self.recent.len() > 3 {
            self.recent.remove(0);
        }
    }

    fn set_rime_input(&mut self) -> bool {
        let raw = &self.raw;
        self.rime.as_mut().is_some_and(|rime| rime.set_input(raw))
    }

    fn clear_rime(&mut self) {
        if let Some(rime) = &mut self.rime {
            rime.clear();
        }
    }

    fn render_rime(&mut self) {
        if self.raw.is_empty() {
            self.word_mode = 0;
            self.rime_editing = false;
        }
        self.update_language_options();
        let snapshot = self.rime.as_ref().and_then(Session::snapshot);
        self.language = Language::Chinese;
        match snapshot {
            Some(snapshot) => {
                self.preedit = snapshot.preedit;
                self.choices = snapshot.choices;
                self.caret = snapshot.caret;
            }
            None => {
                self.preedit = self.raw.clone();
                self.choices.clear();
                self.caret = self.preedit.encode_utf16().count();
            }
        }
        self.correction = None;
        self.correction_language = None;
        self.clamp_page();
        self.output.composition_changed = true;
    }

    fn select_rime_candidate(&mut self, index: usize) {
        match self
            .rime
            .as_mut()
            .and_then(|rime| rime.select_candidate(index))
        {
            Some(commit) if !commit.is_empty() => {
                self.remember_language();
                self.finish(commit);
            }
            _ => {
                self.rime_editing = true;
                self.set_page(0);
                self.render_rime();
            }
        }
    }

    fn update_language_options(&mut self) {
        let mut mask = if self.settings.mode() == 0 {
            crate::language_options(&self.raw)
        } else {
            0
        };
        if self.word_mode != 0 && !self.raw.is_empty() {
            mask |= 1 << self.word_mode;
        }
        mask &= self.settings.enabled();
        self.language_options = (1..=3).filter(|l| mask & (1 << l) != 0).collect();
    }

    fn reset_word(&mut self) {
        self.set_page(0);
        self.rime_editing = false;
        self.using_rime = false;
        self.clear_rime();
        self.refresh();
    }

    /// Tab rotates the word being typed through EN → VI → 中; manual mode keeps the choice.
    fn switch_language(&mut self) {
        let active = self.language as u32;
        let enabled = self.settings.enabled();
        let mut next = active;
        loop {
            next = next % 3 + 1;
            if enabled & (1 << next) != 0 {
                break;
            }
        }
        if next == active {
            return;
        }
        if self.settings.mode() != 0 {
            self.settings.language = next;
            self.settings_changed = true;
        }
        self.word_mode = next;
        self.reset_word();
    }

    /// Shift+Tab toggles automatic detection; turning it off pins the language currently shown.
    fn toggle_automatic_while_typing(&mut self) {
        let enabled = self.settings.enabled();
        if enabled & (enabled - 1) == 0 {
            return;
        }
        if self.settings.mode() != 0 {
            self.settings.automatic = true;
            self.word_mode = 0;
        } else {
            self.settings.language = self.language as u32;
            self.settings.automatic = false;
        }
        self.settings_changed = true;
        self.recent.clear();
        self.reset_word();
    }

    fn refresh(&mut self) {
        if self.raw.is_empty() {
            self.word_mode = 0;
        }
        self.update_language_options();
        let mode = self.settings.mode();
        let mut effective = if self.word_mode != 0 {
            self.word_mode
        } else {
            mode
        };
        let learning = self.settings.learn_chinese;
        if let Some(rime) = &mut self.rime {
            rime.set_learning(learning);
        }
        if self.rime_editing && self.using_rime {
            self.render_rime();
            return;
        }
        if effective == 3 && self.set_rime_input() {
            self.using_rime = true;
            self.render_rime();
            return;
        }
        let mut result = crate::compose_with_context(&self.raw, effective, self.context_language());
        let enabled = self.settings.enabled();
        let detected = result.lines().next().and_then(Language::from_name);
        if effective == 0 && detected.is_some_and(|l| enabled & (1 << l as u32) == 0) {
            // Detection picked a disabled language: fall back to the best enabled guess.
            let options = self
                .language_options
                .iter()
                .fold(0, |mask, &option| mask | (1 << option));
            effective = first_language(if options != 0 { options } else { enabled });
            if effective == 3 && self.set_rime_input() {
                self.using_rime = true;
                self.render_rime();
                return;
            }
            result = crate::compose_with_context(&self.raw, effective, self.context_language());
        }
        let mut lines = result.split('\n');
        self.language = lines
            .next()
            .and_then(Language::from_name)
            .unwrap_or(Language::English);
        self.preedit = lines.next().unwrap_or_default().to_owned();
        if self.language == Language::Chinese && self.set_rime_input() {
            self.using_rime = true;
            self.render_rime();
            return;
        }
        self.using_rime = false;
        self.clear_rime();
        let mut choices: Vec<String> = Vec::new();
        for line in lines {
            let candidate = (self.simplify)(line);
            if !choices.contains(&candidate) {
                choices.push(candidate);
            }
        }
        self.choices = choices;
        self.correction = None;
        self.correction_language = None;
        if self.word_mode == 0 && self.settings.autocorrect {
            if let Some(fix) = crate::correction(&self.raw, mode) {
                let parts: Vec<&str> = fix.split('\n').collect();
                if parts.len() >= 2 {
                    let language = Language::from_name(parts[0]);
                    let value = parts[parts.len().min(3) - 1];
                    self.correction = Some(if language == Some(Language::Chinese) {
                        (self.simplify)(value)
                    } else {
                        value.to_owned()
                    });
                    self.correction_language = language;
                }
            }
        }
        self.clamp_page();
        self.caret = self.preedit.encode_utf16().count();
        self.output.composition_changed = true;
    }

    /// Any page change starts again at that page's first candidate.
    fn set_page(&mut self, page: usize) {
        self.page = page;
        self.highlight = 0;
    }

    fn highlighted_index(&self) -> usize {
        (self.page * 9 + self.highlight).min(self.choices.len().saturating_sub(1))
    }

    fn clamp_page(&mut self) {
        self.set_page(self.page.min(self.choices.len().saturating_sub(1) / 9));
    }

    fn finish(&mut self, text: String) {
        if !self.raw.is_empty() {
            self.output.commit.push_str(&text);
            // Inserting text replaces the composition, so no separate redraw is needed.
            self.output.composition_changed = false;
        }
        self.clear_rime();
        self.using_rime = false;
        self.rime_editing = false;
        self.word_mode = 0;
        self.language_options.clear();
        self.raw.clear();
        self.preedit.clear();
        self.caret = 0;
        self.choices.clear();
        self.set_page(0);
        self.correction = None;
        self.correction_language = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller() -> Controller {
        Controller::new(Settings::default(), None, Box::new(str::to_owned))
    }

    fn typed(controller: &mut Controller, text: &str) -> Output {
        for c in text.chars() {
            let key = if c == ' ' { Key::Space } else { Key::Char(c) };
            controller.handle_key(KeyEvent::new(key));
        }
        controller.take_output()
    }

    #[test]
    fn telex_composes_and_commits() {
        let mut c = controller();
        let output = typed(&mut c, "nhwng");
        assert!(output.composition_changed && output.commit.is_empty());
        assert_eq!(c.language(), Language::Vietnamese);
        assert_eq!(c.preedit(), "nhưng");
        assert_eq!(c.caret(), 5);
        assert_eq!(typed(&mut c, " ").commit, "nhưng");
        assert!(!c.is_composing());
        assert_eq!(c.recent_languages(), [Language::Vietnamese]);
    }

    #[test]
    fn vietnamese_context_and_english() {
        let mut c = controller();
        typed(&mut c, "nhwng ");
        typed(&mut c, "its");
        assert_eq!(c.preedit(), "ít");
        assert_eq!(typed(&mut c, " ").commit, "ít");
        typed(&mut c, "hello");
        assert_eq!(c.language(), Language::English);
        assert_eq!(typed(&mut c, " ").commit, "hello");
    }

    #[test]
    fn fallback_pinyin_candidates() {
        let mut c = controller();
        typed(&mut c, "nihao");
        assert_eq!(c.language(), Language::Chinese);
        assert!(!c.using_rime());
        assert!(!c.choices().is_empty());
        let first = c.choices()[0].clone();
        assert!(c.handle_key(KeyEvent::new(Key::Char('1'))));
        assert_eq!(c.take_output().commit, first);
    }

    #[test]
    fn arrows_move_the_highlighted_candidate() {
        let mut c = controller();
        c.settings.automatic = false;
        c.settings.language = 3;
        typed(&mut c, "shi");
        let count = c.choices().len();
        assert!(count > 9, "more than one page of candidates");
        assert!(c.handle_key(KeyEvent::new(Key::Down)));
        assert_eq!(c.panel(None).highlight, 1);
        assert!(c.handle_key(KeyEvent::new(Key::Up)));
        assert!(
            c.handle_key(KeyEvent::new(Key::Up)),
            "up wraps to the last candidate"
        );
        let panel = c.panel(None);
        assert_eq!(
            (panel.page, panel.highlight),
            ((count - 1) / 9, (count - 1) % 9)
        );
        assert!(
            c.handle_key(KeyEvent::new(Key::Down)),
            "down wraps to the first"
        );
        c.handle_key(KeyEvent::new(Key::Down));
        let second = c.choices()[1].clone();
        assert_eq!(typed(&mut c, " ").commit, second);
        assert_eq!(c.settings.mode(), 3, "arrows never change the language");
        c.settings.automatic = true;
        typed(&mut c, "vieetj");
        assert!(
            !c.handle_key(KeyEvent::new(Key::Down)),
            "no candidates: the app gets the arrow"
        );
        assert_eq!(c.take_output().commit, "việt");
        assert!(
            !c.handle_key(KeyEvent::new(Key::Down)),
            "empty composition passes arrows"
        );
    }

    #[test]
    fn tab_switches_and_shift_tab_pins() {
        let mut c = controller();
        typed(&mut c, "chao");
        assert!(c.handle_key(KeyEvent::new(Key::Tab)));
        assert_eq!(c.language(), Language::Chinese);
        assert!(!c.take_settings_changed());
        let shift_tab = KeyEvent {
            key: Key::Tab,
            shift: true,
            shortcut: false,
        };
        assert!(c.handle_key(shift_tab));
        assert!(c.take_settings_changed());
        assert_eq!(c.settings.mode(), 3);
        assert!(!c.handle_key(KeyEvent::new(Key::Escape)) || !c.is_composing());
        assert!(
            !c.handle_key(KeyEvent::new(Key::Tab)),
            "Tab belongs to the app when idle"
        );
    }

    #[test]
    fn shortcuts_and_other_keys_commit() {
        let mut c = controller();
        typed(&mut c, "vieetj");
        let ctrl = KeyEvent {
            key: Key::Char('c'),
            shift: false,
            shortcut: true,
        };
        assert!(!c.handle_key(ctrl));
        assert_eq!(c.take_output().commit, "việt");
        typed(&mut c, "dda");
        assert!(!c.handle_key(KeyEvent::new(Key::Char('.'))));
        assert_eq!(c.take_output().commit, "đa");
    }

    #[test]
    fn escape_keeps_raw_and_backspace_edits() {
        let mut c = controller();
        typed(&mut c, "tieengs");
        c.handle_key(KeyEvent::new(Key::Backspace));
        assert_eq!(c.preedit(), "tiêng");
        assert!(c.handle_key(KeyEvent::new(Key::Escape)));
        assert_eq!(c.take_output().commit, "tieeng");
    }

    #[test]
    fn disabled_languages_are_skipped() {
        let mut c = controller();
        c.settings.enabled_languages = (1 << 1) | (1 << 2);
        typed(&mut c, "nihao");
        assert_ne!(c.language(), Language::Chinese);
        c.settings.enabled_languages = 1 << 2;
        assert_eq!(c.settings.mode(), 2);
    }

    #[test]
    fn panel_meanings_and_hints() {
        let meanings = Meanings::parse(r#"{"你好": {"vi": "xin chào", "en": "hello"}}"#.as_bytes());
        let mut c = controller();
        typed(&mut c, "nihao");
        let index = c.choices().iter().position(|text| text == "你好").unwrap();
        c.choices.swap(0, index);
        assert!(c.panel(Some(&meanings)).items[0].meaning.is_none());
        c.settings.meaning_language = 1;
        let panel = c.panel(Some(&meanings));
        assert!(panel.show_meanings);
        assert_eq!(panel.items[0].meaning.as_deref(), Some("xin chào"));
        assert!(panel.hints.contains(&"Tab đổi tiếng".to_owned()));
        assert_eq!(panel.language, Language::Chinese);
    }
}
