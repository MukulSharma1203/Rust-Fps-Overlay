# Rust FPS Overlay

An ultra-lightweight, native Win32 FPS counter for Windows 10/11 built in Rust. It uses Event Tracing for Windows (ETW) to read frame times directly from the graphics pipeline with minimal RAM and CPU overhead.

## Features
- **Minimal HUD:** Crisp, small white text pushed flush to the top-right corner with zero background.
- **Per-Game Allowlist:** Native GUI to add/remove specific `.exe` files so the overlay only appears when your chosen games are in focus (plus an **Always On Top** toggle).
- **System Tray Mode:** Closing the settings window (`X`) minimizes it to the system tray. Right-click the Rust gear tray icon to reopen settings, toggle the overlay, or quit.
- **Hotkey Support:** Press `F9` anytime to toggle the overlay on or off.
- **Standalone Portable Binary:** Single `.exe` with an embedded Administrator manifest (required for ETW frame tracing).

## Usage
1. Download `fps-overlay.exe` from the **Releases** page and run it (approve the Administrator prompt).
2. Add your game executable via **Browse...**, typing the `.exe` name, or clicking **Add Last Focused App**.
3. Run your game in **Borderless Windowed** or **Windowed** mode.
4. Click `X` on the settings window to keep it running quietly in the system tray.

Your game list is saved automatically to `%APPDATA%\RustFpsOverlay\games.txt`.

## Building from Source
Requires [Rust](https://rustup.rs/) and the **MSVC C++ Build Tools** + **Windows SDK**.

```powershell
git clone [https://github.com/](https://github.com/)<your-username>/fps-overlay.git
cd fps-overlay
cargo build --release
