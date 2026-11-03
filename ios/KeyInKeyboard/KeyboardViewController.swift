import UIKit

final class KeyboardViewController: UIInputViewController, TextOutput {
    private let composer = Composer()
    private lazy var keyboard = KeyboardView(composer: composer)

    override func viewDidLoad() {
        super.viewDidLoad()
        composer.output = self
        keyboard.onGlobe = { [weak self] view, event in
            self?.handleInputModeList(from: view, with: event)
        }
        keyboard.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(keyboard)
        NSLayoutConstraint.activate([
            keyboard.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            keyboard.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            keyboard.topAnchor.constraint(equalTo: view.topAnchor),
            keyboard.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    // The system's edge gestures on the keyboard window hold back touches on
    // the outer keys (q, a, p, l, shift, backspace, bottom row) until they
    // fail, which reads as lag and reordered or dropped letters when typing
    // fast. Keys need every touch immediately.
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        var recognizers = view.window?.gestureRecognizers ?? []
        var superview = view.superview
        while let current = superview {
            recognizers += current.gestureRecognizers ?? []
            superview = current.superview
        }
        for recognizer in recognizers {
            recognizer.delaysTouchesBegan = false
            recognizer.delaysTouchesEnded = false
        }
    }

    override func viewWillLayoutSubviews() {
        super.viewWillLayoutSubviews()
        keyboard.showsGlobe = needsInputModeSwitchKey
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        composer.commit()
    }

    // The user moved the cursor or the host replaced text: the preedit we typed
    // is no longer where we left it, so stop replacing it. Our own edits also
    // trigger these callbacks and must not end the composition.
    override func selectionDidChange(_ textInput: UITextInput?) { hostChanged() }
    override func textDidChange(_ textInput: UITextInput?) { hostChanged() }

    private func hostChanged() {
        if !composer.owns(textBefore: textDocumentProxy.documentContextBeforeInput) { composer.reset() }
    }

    func insertText(_ text: String) { textDocumentProxy.insertText(text) }
    func deleteBackward() { textDocumentProxy.deleteBackward() }
}
