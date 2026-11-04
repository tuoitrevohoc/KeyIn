import Foundation

/// Where composed text goes: the host text field in the keyboard extension,
/// or a local text view in the container app's playground.
protocol TextOutput: AnyObject {
    func insertText(_ text: String)
    func deleteBackward()
}

/// UI text follows the system language: Vietnamese when the first preferred language
/// is Vietnamese, English otherwise.
let vietnameseUI = Locale.preferredLanguages.first?.hasPrefix("vi") ?? false

func L(_ vietnamese: String, _ english: String) -> String { vietnameseUI ? vietnamese : english }

enum Language: UInt32, CaseIterable {
    case english = 1, vietnamese = 2, chinese = 3

    init?(name: String) {
        switch name {
        case "English": self = .english
        case "Vietnamese": self = .vietnamese
        case "Chinese": self = .chinese
        default: return nil
        }
    }

    var flag: String {
        switch self {
        case .english: return "🇺🇸"
        case .vietnamese: return "🇻🇳"
        case .chinese: return "🇨🇳"
        }
    }
}

/// Port of the macOS controller's composition logic (native/main.m) without
/// hardware key events. The word being typed goes straight into the document
/// and only the part that changed is rewritten. Marked (underlined) text was
/// tried and dropped: hosts reflow the line when it appears and disappears.
/// Pinyin is shown as the raw keystrokes, so typing it only ever appends a
/// letter; the syllable-separated reading is on the candidate bar.
///
/// Keystrokes update `raw` at once; the engine runs once per run-loop turn, so
/// a burst of fast taps costs a single refresh.
final class Composer {
    weak var output: TextOutput?
    var onChange: (() -> Void)?

    /// 0 automatic, otherwise a fixed Language.
    var mode: UInt32 = 0 { didSet { commit() } }
    var autocorrect = true

    private(set) var raw = ""
    private(set) var language: Language = .english
    private(set) var preedit = ""
    private(set) var choices: [String] = []
    private(set) var correction: String?
    private(set) var languageOptions: [Language] = []
    private var correctionLanguage: Language?
    private var wordMode: UInt32 = 0
    /// What this composition has typed into the document so far.
    private var shown = ""
    private var lastWrite = Date.distantPast
    private var recentLanguages: [Language] = []
    private var refreshPending = false

    static let allLetters: UInt32 = (1 << 26) - 1
    /// Letters that can follow `raw` (bit 0 = a), for enlarging likely keys.
    private(set) var nextLetters = allLetters

    var isComposing: Bool { !raw.isEmpty }

    func type(_ letter: String) {
        if raw.count >= 128 { commit() }
        raw += letter
        scheduleRefresh()
    }

    func backspace() {
        guard !raw.isEmpty else { output?.deleteBackward(); return }
        raw.removeLast()
        scheduleRefresh()
    }

    private func scheduleRefresh() {
        guard !refreshPending else { return }
        refreshPending = true
        DispatchQueue.main.async { [weak self] in self?.flush() }
    }

    /// Bring candidates up to date before acting on them.
    private func flush() {
        guard refreshPending else { return }
        refreshPending = false
        refresh()
    }

    /// Space or return: confirm the best reading, like the macOS space key.
    func confirm() {
        flush()
        guard !raw.isEmpty else { return }
        let value: String
        if let correction {
            value = correction
            if let correctionLanguage { language = correctionLanguage }
        } else if let first = choices.first {
            value = first
        } else {
            value = language == .chinese ? raw : preedit
        }
        finish(value)
    }

    func choose(_ index: Int) {
        flush()
        guard choices.indices.contains(index) else { return }
        finish(choices[index])
    }

    /// Keep what is shown and forget the composition (cursor moved, punctuation, dismissal).
    func commit() {
        flush()
        guard !raw.isEmpty else { return }
        finish(language == .chinese ? raw : preedit)
    }

    /// Punctuation or a digit ends the word. Pinyin turns into the highlighted
    /// candidate, as space would; other languages keep what is shown.
    func commitBeforePunctuation() {
        flush()
        if language == .chinese, !choices.isEmpty { confirm() } else { commit() }
    }

    /// Whether the text before the cursor still ends with what we typed. Hosts
    /// report our own edits back asynchronously, so a recent write counts as ours.
    func owns(textBefore: String?) -> Bool {
        shown.isEmpty || Date().timeIntervalSince(lastWrite) < 0.5 || (textBefore ?? "").hasSuffix(shown)
    }

    /// Host text changed underneath us; the shown preedit is no longer ours to replace.
    func reset() {
        refreshPending = false; nextLetters = Self.allLetters
        raw = ""; shown = ""; preedit = ""; choices = []; correction = nil
        correctionLanguage = nil; wordMode = 0; languageOptions = []
        onChange?()
    }

    /// Cycle the current word through the languages it can be read in.
    func stepLanguage() {
        flush()
        guard mode == 0, languageOptions.count > 1 else { return }
        let index = languageOptions.firstIndex(of: language) ?? 0
        wordMode = languageOptions[(index + 1) % languageOptions.count].rawValue
        refresh()
    }

    private var contextLanguage: UInt32 {
        if !raw.isEmpty, language != .english { return language.rawValue }
        return recentLanguages.last?.rawValue ?? 0
    }

    private func refresh() {
        if raw.isEmpty { wordMode = 0 }
        let result = Engine.analyze(raw, mode: wordMode != 0 ? wordMode : mode, previous: contextLanguage,
                                    correctMode: autocorrect && wordMode == 0 ? mode : nil)
        var mask = mode == 0 ? result.options : 0
        if wordMode != 0, !raw.isEmpty { mask |= 1 << wordMode }
        languageOptions = Language.allCases.filter { mask & (1 << $0.rawValue) != 0 }
        let lines = result.composition
        language = lines.first.flatMap(Language.init(name:)) ?? .english
        preedit = lines.count > 1 ? lines[1] : raw
        var seen = Set<String>()
        choices = lines.dropFirst(2).map(simplified).filter { seen.insert($0).inserted }
        correction = nil
        correctionLanguage = nil
        if let fix = result.correction, fix.count >= 2 {
            correctionLanguage = Language(name: fix[0])
            let value = fix.count > 2 ? fix[2] : fix[1]
            correction = correctionLanguage == .chinese ? simplified(value) : value
        }
        nextLetters = raw.isEmpty ? Self.allLetters : Engine.nextLetters(raw)
        show(raw.isEmpty ? "" : language == .chinese ? raw : preedit)
        onChange?()
    }

    private func finish(_ text: String) {
        show(text)
        shown = ""
        if !raw.isEmpty {
            recentLanguages.append(language)
            if recentLanguages.count > 3 { recentLanguages.removeFirst() }
        }
        raw = ""; preedit = ""; choices = []; correction = nil
        correctionLanguage = nil; wordMode = 0; languageOptions = []
        nextLetters = Self.allLetters
        onChange?()
    }

    /// Replace the previously shown text with `text`, touching only the differing suffix.
    private func show(_ text: String) {
        let common = zip(shown, text).prefix { $0 == $1 }.count
        for _ in 0..<(shown.count - common) { output?.deleteBackward() }
        let suffix = String(text.dropFirst(common))
        if !suffix.isEmpty { output?.insertText(suffix) }
        shown = text
        lastWrite = Date()
    }

    // ICU transforms cost ~0.1 ms each and the same candidates recur on every keystroke.
    private var simplifiedCache: [String: String] = [:]

    private func simplified(_ text: String) -> String {
        if let hit = simplifiedCache[text] { return hit }
        let value = text.applyingTransform(StringTransform("Traditional-Simplified"), reverse: false) ?? text
        if simplifiedCache.count > 2000 { simplifiedCache.removeAll() }
        simplifiedCache[text] = value
        return value
    }
}

enum Engine {
    struct Analysis {
        var options: UInt32
        var correction: [String]?
        var composition: [String]
    }

    static func nextLetters(_ raw: String) -> UInt32 { keyin_next_letters(raw) }

    /// One engine call per keystroke: language options, correction and composition.
    static func analyze(_ raw: String, mode: UInt32, previous: UInt32, correctMode: UInt32?) -> Analysis {
        guard let pointer = keyin_analyze(raw, mode, previous, correctMode ?? .max) else {
            return Analysis(options: 0, correction: nil, composition: ["English", raw])
        }
        defer { keyin_free(pointer) }
        let parts = String(cString: pointer).components(separatedBy: "\u{1e}")
        guard parts.count == 3 else { return Analysis(options: 0, correction: nil, composition: ["English", raw]) }
        let correction = parts[1].isEmpty ? nil : parts[1].components(separatedBy: "\n")
        return Analysis(options: UInt32(parts[0]) ?? 0, correction: correction,
                        composition: parts[2].components(separatedBy: "\n"))
    }
}
