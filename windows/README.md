# KeyIn for Windows (experimental)

A Text Services Framework (TSF) input method built from the same Rust engine as the
macOS app. Key handling, language detection, Rime sessions and Chinese meanings live in
the shared `keyin` crate (`src/controller.rs`, `src/rime.rs`, `src/meanings.rs`); this
crate is only the Windows adapter:

| File | Role |
| --- | --- |
| `src/lib.rs` | COM DLL entry points and class factory |
| `src/service.rs` | `ITfTextInputProcessorEx`, key sink, composition and edit sessions |
| `src/candidate.rs` | Candidate window (GDI popup that never takes focus) |
| `src/langbar.rs` | Taskbar input-mode button with the settings menu |
| `src/settings.rs` | Preferences in `HKCU\Software\KeyIn` |
| `src/register.rs` | `regsvr32` / MSI registration (COM class, TSF profile, categories) |
| `src/engine.rs` | Loads `rime.dll`, Rime data and the meanings dictionary next to the DLL |

## Build

On Windows 10/11 x64 with Rust (MSVC toolchain), 7-Zip and the .NET SDK:

```powershell
./windows/build.ps1
```

The script downloads the official librime MSVC build (same version as macOS),
precompiles the Rime data, builds `keyin_tsf.dll` and packages
`build\windows\KeyIn-<version>-windows-x64.msi` with WiX. The `Windows` GitHub
workflow does the same on every push and uploads the MSI as an artifact.

## Install and use

1. Run the MSI. It installs to `C:\Program Files\KeyIn` and registers the input method.
2. **Settings → Time & language → Language & region**: add **Vietnamese** if it is not
   listed, then open its **Language options** and add the **KeyIn** keyboard.
3. Switch to KeyIn with **Win+Space**. The taskbar button shows `A` (Auto) or the pinned
   language; left-click cycles Auto → EN → VI → 中, right-click opens the settings menu.

Typing works as on macOS: Telex for Vietnamese, Pinyin for Chinese, 1–9 / Space to
choose a candidate, ← → to page, ↑↓ to reinterpret the word, Tab to switch language and
Shift+Tab to toggle Auto while a word is being composed. Uninstall from **Installed apps**.

## Known limitations

- Rime runs inside each application process. Its learning user dictionary can only be
  opened by one process at a time, so "Học lựa chọn tiếng Trung" is off by default on
  Windows. Sharing learning across apps needs an out-of-process server (as Weasel does).
- x64 only: librime publishes MSVC builds for x64 and x86. 32-bit applications and
  native ARM64 applications need x86 / ARM64 builds of both librime and this DLL.
- Composition text has no underline yet (no `ITfDisplayAttributeProvider`), and the
  candidate window is not per-monitor-DPI aware.
- Sandboxed apps (Store/AppContainer) cannot write `%APPDATA%\KeyIn\Rime`; there KeyIn
  falls back to the built-in Pinyin candidates.
