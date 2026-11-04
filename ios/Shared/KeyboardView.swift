import UIKit

/// QWERTY keyboard with a candidate bar, shared by the keyboard extension and
/// the container app's playground. The whole view takes touches, and each goes
/// to the nearest key.
final class KeyboardView: UIView {
    enum Key: Equatable {
        case character(String)
        case shift, backspace, layer, letters, globe, space, enter
    }

    private enum Page { case letters, numbers, symbols }

    let composer: Composer
    var showsGlobe = true { didSet { if showsGlobe != oldValue { rebuild() } } }
    var onGlobe: ((UIView, UIEvent) -> Void)?
    var onEnter: (() -> Void)?
    var output: TextOutput? { composer.output }

    private let bar = UIScrollView()
    // Candidate chips are reused across keystrokes: creating buttons for every
    // candidate on every key was the main cost of typing Chinese.
    private var chips: [UIButton] = []
    private var chipActions: [() -> Void] = []
    private let preview = UILabel()
    private let keyArea = UIView()
    private var keyViews: [(key: Key, view: UILabel)] = []
    private var page = Page.letters
    private var shifted = false
    private var capsLock = false
    private var lastShiftTap = Date.distantPast
    private var repeatTimer: Timer?
    private var didRepeat = false
    private let bubble = UILabel()
    private let haptics = UIImpactFeedbackGenerator(style: .light)

    static let barHeight: CGFloat = 44
    static let rowHeight: CGFloat = 54

    init(composer: Composer) {
        self.composer = composer
        super.init(frame: .zero)
        // Touches on fully transparent pixels of a keyboard extension never
        // reach it, which silently lost every tap between two keys.
        backgroundColor = UIColor.white.withAlphaComponent(0.001)
        bar.showsHorizontalScrollIndicator = false
        if #available(iOS 26.0, *) {
            // The default soft edge effect blurs the whole bar inside a keyboard.
            for edge in [bar.topEdgeEffect, bar.bottomEdgeEffect, bar.leftEdgeEffect, bar.rightEdgeEffect] {
                edge.isHidden = true
            }
        }
        preview.textColor = .secondaryLabel
        preview.font = .systemFont(ofSize: 17)
        bar.addSubview(preview)
        addSubview(bar)
        addSubview(keyArea)
        isMultipleTouchEnabled = true
        keyArea.isUserInteractionEnabled = false // touches land on self, one per finger
        bubble.textAlignment = .center
        bubble.font = .systemFont(ofSize: 32)
        bubble.backgroundColor = .systemBackground
        bubble.layer.cornerRadius = 8
        bubble.layer.masksToBounds = true
        bubble.isHidden = true
        addSubview(bubble)
        composer.onChange = { [weak self] in self?.updateBar() }
        rebuild()
        updateBar()
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: CGSize {
        CGSize(width: UIView.noIntrinsicMetric, height: Self.barHeight + Self.rowHeight * 4 + 4)
    }

    // MARK: Layout

    private var rows: [[Key]] {
        switch page {
        case .letters:
            let letters = ["qwertyuiop", "asdfghjkl", "zxcvbnm"].map { row in
                row.map { Key.character(shifted ? String($0).uppercased() : String($0)) }
            }
            return [letters[0], letters[1], [.shift] + letters[2] + [.backspace], bottomRow]
        case .numbers:
            return [
                "1234567890".map { .character(String($0)) },
                ["-", "/", ":", ";", "(", ")", "đ", "&", "@", "\""].map { .character($0) },
                [.layer] + [".", ",", "?", "!", "'"].map { .character($0) } + [.backspace],
                bottomRow,
            ]
        case .symbols:
            return [
                ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="].map { .character($0) },
                ["_", "\\", "|", "~", "<", ">", "€", "$", "¥", "•"].map { .character($0) },
                [.layer] + [".", ",", "?", "!", "'"].map { .character($0) } + [.backspace],
                bottomRow,
            ]
        }
    }

    private var bottomRow: [Key] {
        [page == .letters ? .layer : .letters] + (showsGlobe ? [.globe] : []) + [.space, .enter]
    }

    private func rebuild() {
        let keys = rows.flatMap { $0 }
        if keys.count == keyViews.count {
            // Same layout (shift, caps lock, mode label): relabel in place.
            keyViews = zip(keys, keyViews).map { key, old in
                configure(old.view, for: key)
                return (key, old.view)
            }
            setNeedsLayout()
            return
        }
        keyViews.forEach { $0.view.removeFromSuperview() }
        keyViews = keys.map { key in
            let label = UILabel()
            label.textAlignment = .center
            label.layer.cornerRadius = 6
            label.layer.masksToBounds = true
            label.isUserInteractionEnabled = false
            configure(label, for: key)
            keyArea.addSubview(label)
            return (key, label)
        }
        setNeedsLayout()
    }

    private func configure(_ label: UILabel, for key: Key) {
        let special = !(key == .space || { if case .character = key { return true }; return false }())
        label.backgroundColor = special ? .systemGray3 : .systemBackground
        label.textColor = .label
        label.font = .systemFont(ofSize: special ? 16 : 22)
        switch key {
        case .character(let text): label.text = text
        case .shift: label.text = capsLock ? "⇪" : (shifted ? "⬆︎" : "⇧")
        case .backspace: label.text = "⌫"
        case .layer: label.text = page == .letters ? "123" : (page == .numbers ? "#+=" : "123")
        case .letters: label.text = "ABC"
        case .globe: label.text = "🌐"
        case .space: label.text = composer.mode == 0 ? "KeyIn" : "KeyIn · \(Language(rawValue: composer.mode)?.flag ?? "")"
            label.font = .systemFont(ofSize: 15); label.textColor = .secondaryLabel
        case .enter: label.text = "↵"
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        bar.frame = CGRect(x: 0, y: 0, width: bounds.width, height: Self.barHeight)
        layoutBar()
        keyArea.frame = CGRect(x: 0, y: Self.barHeight, width: bounds.width, height: bounds.height - Self.barHeight)

        let gap: CGFloat = 6, side: CGFloat = 3
        let unit = (bounds.width - side * 2) / 10
        var index = 0
        for (r, row) in rows.enumerated() {
            let widths = row.map { width(of: $0, in: row, unit: unit) }
            let total = widths.reduce(0, +)
            var x = (bounds.width - total) / 2
            for w in widths {
                keyViews[index].view.frame = CGRect(x: x + gap / 2, y: CGFloat(r) * Self.rowHeight + 5,
                                                    width: w - gap, height: Self.rowHeight - 10)
                x += w; index += 1
            }
        }
    }

    /// Candidates change on every keystroke; the keys do not.
    private func layoutBar() {
        var x: CGFloat = 6
        for chip in chips.prefix(chipActions.count) {
            let width = (chip.titleLabel?.intrinsicContentSize.width ?? 0) + 24
            chip.frame = CGRect(x: x, y: (Self.barHeight - 34) / 2, width: width, height: 34)
            x += width + 4
        }
        if !preview.isHidden {
            let width = preview.intrinsicContentSize.width
            preview.frame = CGRect(x: x, y: 0, width: width, height: Self.barHeight)
            x += width
        }
        bar.contentSize = CGSize(width: x + 6, height: Self.barHeight)
    }

    private func width(of key: Key, in row: [Key], unit: CGFloat) -> CGFloat {
        let fixed = row.filter { $0 != .space }.count
        switch key {
        case .shift, .backspace: return unit * 1.5
        case .layer, .letters, .globe: return unit * 1.25
        case .enter: return unit * 2
        case .space:
            let others = row.filter { $0 != .space }.map { width(of: $0, in: row, unit: unit) }.reduce(0, +)
            return unit * 10 - others
        case .character:
            // The letters row with shift/backspace shares the remaining width.
            if row.contains(.shift) || (row.contains(.layer) && fixed > 5) {
                let specials = unit * 3
                return (unit * 10 - specials) / CGFloat(row.count - 2)
            }
            return unit
        }
    }

    // MARK: Touches

    /// Fingers aiming at the top row often land a few points into the
    /// candidate bar; anything below the chips still counts as a key.
    private static let topSlop: CGFloat = 10

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        if point.y >= keyArea.frame.minY - Self.topSlop, bounds.contains(point) { return self }
        return super.hitTest(point, with: event)
    }

    // Fast typing overlaps touches: the next finger lands before the previous
    // one lifts. Each touch is tracked on its own, and a new touch commits the
    // ones still down, in order, so no keystroke is lost or reordered. The key
    // is chosen where the finger lands: fingers drift while lifting, and the
    // landing point is what the typist aimed at.
    private var active: [(touch: UITouch, key: Key, view: UILabel)] = []

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches.sorted(by: { $0.timestamp < $1.timestamp }) {
            active.forEach(release)
            active.removeAll()
            var point = touch.location(in: keyArea)
            point.y = max(point.y, 0)
            guard let target = keyAt(point) else { continue }
            target.view.alpha = 0.5
            active.append((touch, target.key, target.view))
            UIDevice.current.playInputClick()
            haptics.impactOccurred()
            haptics.prepare()
            showBubble(for: target)
            if target.key == .backspace { startRepeating() }
            if target.key == .globe, let event { onGlobe?(target.view, event) }
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        let ended = active.filter { touches.contains($0.touch) }
        active.removeAll { touches.contains($0.touch) }
        ended.forEach(release)
    }

    // System gestures near the screen edges cancel touches that were meant as
    // keystrokes; treat a cancel like a lift so the key still registers.
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchesEnded(touches, with: event)
    }

    private func release(_ entry: (touch: UITouch, key: Key, view: UILabel)) {
        entry.view.alpha = 1
        bubble.isHidden = true
        stopRepeating()
        // A held backspace already deleted while repeating.
        if entry.key == .backspace, didRepeat { return }
        press(entry.key)
    }

    private func showBubble(for target: (key: Key, view: UILabel)) {
        guard case .character(let text) = target.key else { bubble.isHidden = true; return }
        let frame = keyArea.convert(target.view.frame, to: self)
        let height = frame.height * 1.15
        bubble.text = text
        bubble.frame = CGRect(x: frame.midX - (frame.width + 16) / 2, y: frame.minY - height - 2,
                              width: frame.width + 16, height: height)
        bubble.isHidden = false
    }

    /// Thumbs land below what they aim at on the upper rows and above it on the
    /// bottom row, so rows are hit-tested where fingers actually land. The
    /// stops map measured row boundaries (62, 118, 160) to the drawn ones.
    private static func aimedY(_ y: CGFloat) -> CGFloat {
        let stops: [(landed: CGFloat, drawn: CGFloat)] = [(0, 0), (62, 54), (118, 108), (160, 162), (216, 216)]
        for (low, high) in zip(stops, stops.dropFirst()) where y <= high.landed {
            return low.drawn + (y - low.landed) * (high.drawn - low.drawn) / (high.landed - low.landed)
        }
        return y
    }

    /// The key under `point`. Letters that can follow what is being typed get a
    /// slightly larger target, so a touch near the border of two keys goes to
    /// the one that makes sense ("zh" then a/e/i/o/u rather than their neighbours).
    private func keyAt(_ landing: CGPoint) -> (key: Key, view: UILabel)? {
        let point = CGPoint(x: landing.x, y: Self.aimedY(landing.y))
        guard let plain = keyViews.min(by: { distance(point, $0.view.frame) < distance(point, $1.view.frame) }) else {
            return nil
        }
        let likely = composer.nextLetters
        guard page == .letters, composer.isComposing, likely != Composer.allLetters,
              case .character = plain.key else { return plain }
        func score(_ entry: (key: Key, view: UILabel)) -> CGFloat? {
            guard case .character(let text) = entry.key, let c = text.lowercased().unicodeScalars.first,
                  c.isASCII, (97...122).contains(c.value) else { return nil }
            let frame = entry.view.frame
            let dx = (point.x - frame.midX) / frame.width, dy = (point.y - frame.midY) / frame.height
            let d = (dx * dx + dy * dy).squareRoot()
            guard d < 0.9 else { return nil }
            return likely & (1 << (c.value - 97)) != 0 ? d * 0.7 : d
        }
        return keyViews.compactMap { entry in score(entry).map { (entry, $0) } }
            .min { $0.1 < $1.1 }?.0 ?? plain
    }

    private func distance(_ p: CGPoint, _ r: CGRect) -> CGFloat {
        let dx = max(r.minX - p.x, 0, p.x - r.maxX)
        let dy = max(r.minY - p.y, 0, p.y - r.maxY)
        return dx * dx + dy * dy
    }

    private func startRepeating() {
        didRepeat = false
        repeatTimer = Timer.scheduledTimer(withTimeInterval: 0.45, repeats: false) { [weak self] _ in
            self?.didRepeat = true
            self?.composer.backspace()
            self?.repeatTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { _ in
                self?.composer.backspace()
            }
        }
    }

    private func stopRepeating() { repeatTimer?.invalidate(); repeatTimer = nil }

    private func press(_ key: Key) {
        switch key {
        case .character(let text):
            if let c = text.first, text.count == 1, (c.isASCII && c.isLetter) || (c == "'" && composer.isComposing) {
                composer.type(text)
            } else {
                composer.commitBeforePunctuation()
                output?.insertText(text)
            }
            if shifted, !capsLock { shifted = false; rebuild() }
        case .shift:
            let now = Date()
            if now.timeIntervalSince(lastShiftTap) < 0.3 { capsLock = true; shifted = true }
            else { capsLock = false; shifted.toggle() }
            lastShiftTap = now
            rebuild()
        case .backspace:
            composer.backspace()
        case .layer:
            page = page == .numbers ? .symbols : .numbers
            rebuild()
        case .letters:
            page = .letters
            rebuild()
        case .globe:
            break
        case .space:
            composer.confirm()
            output?.insertText(" ")
            if page != .letters { page = .letters; rebuild() }
        case .enter:
            composer.settle()
            if composer.isComposing { composer.confirm() } else if let onEnter { onEnter() } else { output?.insertText("\n") }
        }
    }

    // MARK: Candidate bar

    private enum ChipStyle { case plain, selected, active, flag }

    private func updateBar() {
        var items: [(title: String, style: ChipStyle, action: () -> Void)] = []
        preview.isHidden = true
        if composer.isComposing {
            items.append((composer.language.flag, .flag, { [weak self] in self?.composer.stepLanguage() }))
            // The candidate that space would insert is shown in bold.
            if let correction = composer.correction {
                items.append(("✓ " + correction, .selected, { [weak self] in self?.composer.confirm() }))
            }
            for (i, choice) in composer.choices.prefix(30).enumerated() {
                let style: ChipStyle = i == 0 && composer.correction == nil ? .selected : .plain
                items.append((choice, style, { [weak self] in self?.composer.choose(i) }))
            }
            if composer.choices.isEmpty, composer.correction == nil {
                preview.text = composer.preedit
                preview.isHidden = false
            }
        } else {
            let modes = [(UInt32(0), "Tự động")] + Language.allCases.map { ($0.rawValue, $0.flag) }
            for (mode, title) in modes {
                items.append((title, composer.mode == mode ? .active : .plain, { [weak self] in
                    self?.composer.mode = mode
                    self?.updateBar()
                    self?.rebuild()
                }))
            }
        }
        while chips.count < items.count {
            let chip = UIButton(type: .custom)
            chip.layer.cornerRadius = 17
            chip.tag = chips.count
            chip.addTarget(self, action: #selector(chipTapped(_:)), for: .touchUpInside)
            bar.addSubview(chip)
            chips.append(chip)
        }
        for (chip, item) in zip(chips, items) {
            chip.isHidden = false
            chip.setTitle(item.title, for: .normal)
            chip.titleLabel?.font = item.style == .selected ? .boldSystemFont(ofSize: 18) : .systemFont(ofSize: item.style == .flag ? 22 : 18)
            chip.setTitleColor(item.style == .active ? .white : .label, for: .normal)
            switch item.style {
            case .active: chip.backgroundColor = tintColor
            case .selected: chip.backgroundColor = .systemBackground
            case .plain, .flag: chip.backgroundColor = .clear
            }
        }
        chips.dropFirst(items.count).forEach { $0.isHidden = true }
        chipActions = items.map(\.action)
        bar.setContentOffset(.zero, animated: false)
        layoutBar()
    }

    @objc private func chipTapped(_ chip: UIButton) {
        if chipActions.indices.contains(chip.tag) { chipActions[chip.tag]() }
    }
}
