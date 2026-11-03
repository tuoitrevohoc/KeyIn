//! Controller + librime integration. Runs when KEYIN_RIME_LIB (librime shared library)
//! and KEYIN_RIME_SHARED (prebuilt Rime data directory) are set, e.g. by
//! scripts/test-native.sh; otherwise it is skipped.
use keyin::{
    controller::{Controller, Key, KeyEvent, Language, Settings},
    rime::{self, Session},
};
use std::path::PathBuf;

fn start() -> bool {
    let (Some(library), Some(shared)) = (
        std::env::var_os("KEYIN_RIME_LIB"),
        std::env::var_os("KEYIN_RIME_SHARED"),
    ) else {
        eprintln!("KEYIN_RIME_LIB / KEYIN_RIME_SHARED not set; skipping librime checks");
        return false;
    };
    let user = std::env::temp_dir().join(format!("keyin-rime-test-{}", std::process::id()));
    let api = unsafe { rime::load_library(&PathBuf::from(library)) };
    assert!(!api.is_null(), "load librime");
    assert!(
        unsafe { rime::start(api, &PathBuf::from(shared), &user) },
        "Rime startup"
    );
    true
}

fn typed(controller: &mut Controller, text: &str) -> String {
    for c in text.chars() {
        let key = if c == ' ' { Key::Space } else { Key::Char(c) };
        controller.handle_key(KeyEvent::new(key));
    }
    controller.take_output().commit
}

#[test]
fn rime_session_and_controller() {
    if !start() {
        return;
    }
    let mut engine = Session::new().expect("session/schema");
    for (raw, expected) in [
        ("kankan", "看看"),
        ("zenmeyang", "怎么样"),
        ("nihao", "你好"),
        ("nihaolema", "你好了吗"),
    ] {
        engine.clear();
        assert!(engine.set_input(raw), "{raw}");
        let choices = engine.snapshot().unwrap().choices;
        let index = choices
            .iter()
            .position(|c| c == expected)
            .unwrap_or_else(|| panic!("candidate {raw}"));
        assert_eq!(engine.select_candidate(index).as_deref(), Some(expected));
    }
    engine.clear();
    assert!(engine.set_input("nihao"));
    engine.process_key(0xff51);
    assert!(engine.caret() < 5, "caret movement");
    engine.process_key(0xff08);
    assert_eq!(engine.input().len(), 4, "delete in middle");
    engine.clear();
    assert!(engine.set_input("nihao"));
    let ni = engine
        .snapshot()
        .unwrap()
        .choices
        .iter()
        .position(|c| c == "你")
        .expect("partial segment");
    assert_eq!(
        engine.select_candidate(ni),
        None,
        "partial selection keeps composing"
    );
    assert!(engine.snapshot().unwrap().preedit.starts_with('你'));
    assert_eq!(engine.select_candidate(0).as_deref(), Some("你好"));
    engine.set_learning(false);
    assert!(!engine.learning());
    assert!(engine.set_input("zenmeyang"));
    assert_eq!(
        engine.snapshot().unwrap().choices[0],
        "怎么样",
        "private schema"
    );
    engine.clear();
    engine.set_learning(true);
    assert!(
        Session::new().unwrap().input().is_empty(),
        "isolated sessions"
    );

    let mut c = Controller::new(Settings::default(), Session::new(), Box::new(str::to_owned));
    assert_eq!(typed(&mut c, "nihao "), "你好");
    typed(&mut c, "ni");
    assert!(c.using_rime(), "short Pinyin follows Chinese context");
    c.handle_key(KeyEvent::new(Key::Escape));
    c.take_output();
    typed(&mut c, "nhwng");
    assert_eq!((c.language(), c.preedit()), (Language::Vietnamese, "nhưng"));
    typed(&mut c, " ");
    typed(&mut c, "zenmeyang");
    assert!(c.using_rime());
    assert_eq!(c.choices()[0], "怎么样");
    assert_eq!(typed(&mut c, " "), "怎么样");
    typed(&mut c, "ne");
    assert_eq!(c.choices()[0], "呢", "ne continues Chinese");
    assert_eq!(typed(&mut c, " "), "呢");
    typed(&mut c, "nhlm");
    assert_eq!(c.choices()[0], "你好了吗", "initials keep Chinese context");
    assert_eq!(typed(&mut c, " "), "你好了吗");
    typed(&mut c, "hello");
    assert_eq!(
        c.language(),
        Language::English,
        "strong English still switches"
    );
    assert!(c.recent_languages().len() <= 3);
    c.deactivate();
    c.take_output();
    typed(&mut c, "chao");
    assert_eq!(c.language(), Language::Vietnamese, "initial interpretation");
    c.handle_key(KeyEvent::new(Key::Down));
    assert!(c.using_rime() && c.word_mode() == 3, "down enters Chinese");
    assert_eq!(c.raw(), "chao");
}
