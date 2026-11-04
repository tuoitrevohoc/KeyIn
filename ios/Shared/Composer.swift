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
/// Pinyin shows its syllable separators, as on the desktop.
///
/// Keystrokes update `raw` at once on the main thread, which then only hands
/// the input to `EngineWorker`, so touches are never held up by the engine.
/// The answer comes back as a `generation`-tagged reply; anything acting on the
/// word (space, punctuation, a candidate) first calls `settle()` to wait for it.
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
    /// The word's reading in the other languages it could be.
    private(set) var alternatives: [EngineWorker.Alternative] = []
    private(set) var correctionLanguage: Language?
    private var wordMode: UInt32 = 0
    /// What this composition has typed into the document so far.
    private var shown = ""
    private var lastKey = Date.distantPast
    private var recentLanguages: [Language] = []
    private let worker = EngineWorker()
    /// Bumped by every change to the input; `applied` catches up as replies arrive.
    private var generation = 0
    private var applied = 0
    private var request: EngineWorker.Request?

    static let allLetters: UInt32 = (1 << 26) - 1
    /// Letters that can follow `raw` (bit 0 = a), for enlarging likely keys.
    private(set) var nextLetters = allLetters

    var isComposing: Bool { !raw.isEmpty }

    init() {
        worker.onReply = { [weak self] in self?.apply($0) }
    }

    deinit { worker.stop() }

    func type(_ letter: String) {
        if raw.count >= 128 { commit() }
        raw += letter
        scheduleRefresh()
    }

    func backspace() {
        guard !raw.isEmpty else {
            settle()
            output?.deleteBackward()
            return
        }
        raw.removeLast()
        scheduleRefresh()
    }

    private func scheduleRefresh() {
        if raw.isEmpty { wordMode = 0 }
        lastKey = Date()
        let settled = applied == generation
        generation += 1
        let next = EngineWorker.Request(
            generation: generation, raw: raw, mode: wordMode != 0 ? wordMode : mode,
            previous: recentLanguages.last?.rawValue ?? 0, carried: settled ? language : nil,
            offersAlternatives: mode == 0,
            correctMode: autocorrect && wordMode == 0 ? mode : nil)
        request = next
        worker.submit(next)
    }

    /// Bring the composition up to date before acting on it. The wait is the
    /// length of one analysis, well under a millisecond.
    func settle() {
        guard applied != generation, let request else { return }
        apply(worker.reply(for: request))
    }

    /// Space or return: confirm the best reading, like the macOS space key.
    func confirm() {
        settle()
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

    /// Take another language's reading. Chinese still needs a candidate picked.
    func chooseAlternative(_ index: Int) {
        settle()
        guard alternatives.indices.contains(index) else { return }
        let alternative = alternatives[index]
        if alternative.language == .chinese {
            wordMode = alternative.language.rawValue
            scheduleRefresh()
        } else {
            language = alternative.language
            finish(alternative.text)
        }
    }

    func choose(_ index: Int) {
        settle()
        guard choices.indices.contains(index) else { return }
        finish(choices[index])
    }

    /// Keep what is shown and forget the composition (cursor moved, punctuation, dismissal).
    func commit() {
        settle()
        guard !raw.isEmpty else { return }
        finish(language == .chinese ? raw : preedit)
    }

    /// Punctuation or a digit ends the word. Pinyin turns into the highlighted
    /// candidate, as space would; other languages keep what is shown.
    func commitBeforePunctuation() {
        settle()
        if language == .chinese, !choices.isEmpty { confirm() } else { commit() }
    }

    /// Whether the text before the cursor can still be ours. Hosts report our own
    /// edits late and some report no context at all, so while the typist is
    /// active, or when the host says nothing, the composition is kept: wrongly
    /// ending it would lose the letters in flight.
    func owns(textBefore: String?) -> Bool {
        guard !shown.isEmpty, applied == generation, Date().timeIntervalSince(lastKey) > 1,
              let textBefore, !textBefore.isEmpty else { return true }
        return textBefore.hasSuffix(shown)
    }

    /// Host text changed underneath us; the shown preedit is no longer ours to replace.
    func reset() {
        generation += 1; applied = generation; nextLetters = Self.allLetters
        raw = ""; shown = ""; preedit = ""; choices = []; correction = nil
        correctionLanguage = nil; wordMode = 0; languageOptions = []; alternatives = []
        onChange?()
    }

    /// Cycle the current word through the languages it can be read in.
    func stepLanguage() {
        settle()
        guard !raw.isEmpty else { return }
        let all = Language.allCases
        let index = all.firstIndex(of: language) ?? 0
        wordMode = all[(index + 1) % all.count].rawValue
        scheduleRefresh()
    }

    /// Take over the worker's answer, unless the input has moved on since.
    private func apply(_ reply: EngineWorker.Reply) {
        guard reply.generation == generation, applied != generation else { return }
        applied = generation
        var mask = mode == 0 ? reply.options : 0
        if wordMode != 0, !raw.isEmpty { mask |= 1 << wordMode }
        languageOptions = Language.allCases.filter { mask & (1 << $0.rawValue) != 0 }
        language = reply.language
        preedit = reply.preedit
        choices = reply.choices
        correction = reply.correction
        correctionLanguage = reply.correctionLanguage
        nextLetters = reply.nextLetters
        alternatives = reply.alternatives
        show(raw.isEmpty ? "" : preedit)
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
        correctionLanguage = nil; wordMode = 0; languageOptions = []; alternatives = []
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
