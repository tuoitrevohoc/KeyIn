import Foundation

/// Runs the engine on one dedicated thread so the main thread only handles
/// touches and the document. It must be a single, long-lived thread rather
/// than a dispatch queue: the engine keeps per-thread caches, including the
/// table that makes each new Pinyin letter cheap, and a queue may hop threads.
///
/// Every keystroke is analysed in order, because detecting a word's language
/// depends on the reading of its earlier letters; only the newest answer goes
/// back to the main thread. An analysis takes well under a millisecond, far
/// less than the gap between taps, so no backlog builds up.
final class EngineWorker {
    struct Request {
        var generation: Int
        var raw: String
        var mode: UInt32
        /// Language of the last finished word, 0 if none.
        var previous: UInt32
        /// The language so far when the caller knows it, that is, when no
        /// other request is still in flight. Otherwise the worker tracks it.
        var carried: Language?
        var correctMode: UInt32?
    }

    struct Reply {
        var generation: Int
        var options: UInt32
        var language: Language
        var preedit: String
        var choices: [String]
        var correction: String?
        var correctionLanguage: Language?
        var nextLetters: UInt32
    }

    /// Called on the main thread.
    var onReply: ((Reply) -> Void)?

    private let condition = NSCondition()
    private var pending: [Request] = []
    private var latest: Reply?
    private var stopped = false
    // ICU transforms cost ~0.1 ms each and the same candidates recur on every keystroke.
    private var simplifiedCache: [String: String] = [:]
    private var language = Language.english

    init() {
        let thread = Thread { [self] in run() }
        thread.name = "KeyIn engine"
        thread.qualityOfService = .userInteractive
        thread.start()
    }

    func submit(_ request: Request) {
        condition.lock()
        pending.append(request)
        condition.signal()
        condition.unlock()
    }

    /// The thread keeps the worker alive until this is called.
    func stop() {
        condition.lock()
        stopped = true
        condition.signal()
        condition.unlock()
    }

    /// The answer to the newest request, waiting for the worker if needed. If
    /// it is stuck the work is done here, at the cost of a slower Pinyin lookup.
    func reply(for request: Request) -> Reply {
        condition.lock()
        defer { condition.unlock() }
        let deadline = Date(timeIntervalSinceNow: 0.25)
        while latest?.generation != request.generation {
            if !condition.wait(until: deadline) {
                return Self.analyze(request, language: request.carried ?? .english) { Self.simplify($0) }
            }
        }
        return latest!
    }

    private func run() {
        while true {
            condition.lock()
            while pending.isEmpty, !stopped { condition.wait() }
            guard !stopped else {
                condition.unlock()
                return
            }
            let request = pending.removeFirst()
            condition.unlock()

            let reply = Self.analyze(request, language: request.carried ?? language) { cachedSimplify($0) }
            language = reply.language

            condition.lock()
            latest = reply
            let newest = pending.isEmpty
            condition.broadcast()
            condition.unlock()
            if newest {
                DispatchQueue.main.async { [weak self] in self?.onReply?(reply) }
            }
        }
    }

    private func cachedSimplify(_ text: String) -> String {
        if let hit = simplifiedCache[text] { return hit }
        let value = Self.simplify(text)
        if simplifiedCache.count > 2000 { simplifiedCache.removeAll() }
        simplifiedCache[text] = value
        return value
    }

    private static func simplify(_ text: String) -> String {
        text.applyingTransform(StringTransform("Traditional-Simplified"), reverse: false) ?? text
    }

    private static func analyze(_ request: Request, language: Language, simplified: (String) -> String) -> Reply {
        let raw = request.raw
        let previous = !raw.isEmpty && language != .english ? language.rawValue : request.previous
        let result = Engine.analyze(raw, mode: request.mode, previous: previous, correctMode: request.correctMode)
        let lines = result.composition
        var seen = Set<String>()
        var correction: String?
        var correctionLanguage: Language?
        if let fix = result.correction, fix.count >= 2 {
            correctionLanguage = Language(name: fix[0])
            let value = fix.count > 2 ? fix[2] : fix[1]
            correction = correctionLanguage == .chinese ? simplified(value) : value
        }
        return Reply(
            generation: request.generation,
            options: result.options,
            language: lines.first.flatMap(Language.init(name:)) ?? .english,
            preedit: lines.count > 1 ? lines[1] : raw,
            choices: lines.dropFirst(2).map(simplified).filter { seen.insert($0).inserted },
            correction: correction,
            correctionLanguage: correctionLanguage,
            nextLetters: raw.isEmpty ? Composer.allLetters : Engine.nextLetters(raw))
    }
}
