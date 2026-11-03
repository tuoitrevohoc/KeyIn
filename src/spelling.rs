// Shared by the engine and build.rs, which precomputes the dictionaries.

const VOWELS: [&str; 12] = [
    "aáàảãạ",
    "ăắằẳẵặ",
    "âấầẩẫậ",
    "eéèẻẽẹ",
    "êếềểễệ",
    "iíìỉĩị",
    "oóòỏõọ",
    "ôốồổỗộ",
    "ơớờởỡợ",
    "uúùủũụ",
    "ưứừửữự",
    "yýỳỷỹỵ",
];

fn base(c: char) -> char {
    let c = c.to_lowercase().next().unwrap();
    VOWELS
        .iter()
        .find(|s| s.contains(c))
        .map_or(c, |s| s.chars().next().unwrap())
}

fn spelling_key(text: &str) -> String {
    // Treat hòa/hoà and thủy/thuỷ alike without discarding the tone itself.
    let mut tone = 0;
    let mut key = String::new();
    for c in text.to_lowercase().chars() {
        if let Some(row) = VOWELS.iter().find(|row| row.contains(c)) {
            let index = row.chars().position(|v| v == c).unwrap();
            if index != 0 {
                tone = index;
            }
        }
        key.push(base(c));
    }
    key.push(char::from_digit(tone as u32, 10).unwrap());
    key
}
