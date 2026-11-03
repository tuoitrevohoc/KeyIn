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
                Section("Bật bàn phím KeyIn") {
                    Text("Cài đặt › Cài đặt chung › Bàn phím › Bàn phím › Thêm bàn phím mới… › KeyIn")
                    Button("Mở Cài đặt") {
                        if let url = URL(string: UIApplication.openSettingsURLString) {
                            UIApplication.shared.open(url)
                        }
                    }
                }
                Section("Thử ngay trong app") {
                    Playground().frame(height: 380)
                        .listRowInsets(EdgeInsets())
                }
                Section("Cách gõ") {
                    Text("Gõ Telex như bình thường; KeyIn tự nhận ra tiếng Việt, tiếng Anh hoặc pinyin. Chạm nhãn ngôn ngữ trên thanh gợi ý để đổi cách đọc của từ đang gõ.")
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
