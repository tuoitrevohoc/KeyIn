import SwiftUI
import UIKit

@main
struct KeyInApp: App {
    var body: some Scene {
        WindowGroup { ContentView() }
    }
}

struct ContentView: View {
    var body: some View {
        NavigationStack {
            List {
                Section(L("Bật bàn phím KeyIn", "Turn on the KeyIn keyboard")) {
                    Text(L("Cài đặt › Cài đặt chung › Bàn phím › Bàn phím › Thêm bàn phím mới… › KeyIn",
                           "Settings › General › Keyboard › Keyboards › Add New Keyboard… › KeyIn"))
                    Button(L("Mở Cài đặt", "Open Settings")) {
                        if let url = URL(string: UIApplication.openSettingsURLString) {
                            UIApplication.shared.open(url)
                        }
                    }
                }
                Section(L("Thử ngay trong app", "Try it here")) {
                    Playground().frame(height: 380)
                        .listRowInsets(EdgeInsets())
                }
                Section(L("Cách gõ", "How to type")) {
                    Text(L("Gõ Telex như bình thường; KeyIn tự nhận ra tiếng Việt, tiếng Anh hoặc pinyin. Chạm nhãn ngôn ngữ trên thanh gợi ý để đổi cách đọc của từ đang gõ.",
                           "Type as usual (Vietnamese with Telex); KeyIn recognises Vietnamese, English or pinyin by itself. Tap the language label on the suggestion bar to change how the current word is read."))
                }
            }
            .navigationTitle("KeyIn")
        }
    }
}

/// The same keyboard view the extension uses, wired to a local text view so the
/// engine can be tried without enabling the keyboard.
struct Playground: UIViewRepresentable {
    func makeUIView(context: Context) -> UIView { PlaygroundView() }
    func updateUIView(_ uiView: UIView, context: Context) {}
}

final class PlaygroundView: UIView, TextOutput {
    private let text = UITextView()
    private let composer = Composer()
    private lazy var keyboard = KeyboardView(composer: composer)

    init() {
        super.init(frame: .zero)
        composer.output = self
        keyboard.showsGlobe = false
        text.font = .systemFont(ofSize: 20)
        text.inputView = UIView() // keep the system keyboard away
        text.backgroundColor = .secondarySystemBackground
        backgroundColor = .systemGray5
        addSubview(text)
        addSubview(keyboard)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func layoutSubviews() {
        super.layoutSubviews()
        let height = keyboard.intrinsicContentSize.height
        text.frame = CGRect(x: 0, y: 0, width: bounds.width, height: bounds.height - height)
        keyboard.frame = CGRect(x: 0, y: bounds.height - height, width: bounds.width, height: height)
    }

    func insertText(_ string: String) { text.insertText(string) }
    func deleteBackward() { text.deleteBackward() }
}
