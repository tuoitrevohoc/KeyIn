// Types a script through the real controller and prints the panel after every key as JSON lines.
use keyin::{
    controller::{Controller, Key, KeyEvent, Settings},
    meanings::Meanings,
    rime::{self, Session},
};
use std::path::PathBuf;

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn main() {
    let lib = std::env::var("KEYIN_RIME_LIB").unwrap();
    let shared = std::env::var("KEYIN_RIME_SHARED").unwrap();
    let user = std::env::temp_dir().join(format!("keyin-trace-{}", std::process::id()));
    let api = unsafe { rime::load_library(&PathBuf::from(lib)) };
    assert!(unsafe { rime::start(api, &PathBuf::from(&shared), &user) });
    let meanings = Meanings::load_shared(&PathBuf::from(std::env::var("KEYIN_MEANINGS").unwrap()));
    let settings = Settings {
        meaning_language: std::env::var("KEYIN_ML")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        ..Default::default()
    };
    let mut c = Controller::new(settings, Session::new(), Box::new(str::to_owned));
    let script = std::env::args().nth(1).unwrap();
    let mut doc = String::new();
    for ch in script.chars() {
        let key = match ch {
            ' ' => Key::Space,
            '\t' => Key::Tab,
            '\n' => Key::Return,
            c => Key::Char(c),
        };
        let handled = c.handle_key(KeyEvent::new(key));
        let out = c.take_output();
        doc.push_str(&out.commit);
        if !handled && ch != '\t' {
            doc.push(ch);
        }
        let p = c.panel(Some(meanings));
        let items: Vec<String> = p
            .items
            .iter()
            .map(|i| {
                format!(
                    "[\"{}\",\"{}\"]",
                    esc(&i.text),
                    esc(i.meaning.as_deref().unwrap_or(""))
                )
            })
            .collect();
        println!("{{\"k\":\"{}\",\"doc\":\"{}\",\"pre\":\"{}\",\"lang\":\"{}\",\"hints\":[{}],\"items\":[{}],\"pages\":{}}}",
            esc(&ch.to_string()), esc(&doc), esc(c.preedit()), p.language.badge(),
            p.hints.iter().map(|h| format!("\"{}\"", esc(h))).collect::<Vec<_>>().join(","), items.join(","), p.pages);
    }
}
