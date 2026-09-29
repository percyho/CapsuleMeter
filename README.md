# CapsuleMeter — Codex Usage Capsule

**English** | [中文](中文文档.md)

[![Vue 3](https://img.shields.io/badge/Vue-3-42b883?logo=vuedotjs&logoColor=white)](https://vuejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5-3178c6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Vite](https://img.shields.io/badge/Vite-6-646cff?logo=vite&logoColor=white)](https://vite.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24c8db?logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-2021-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
![License](https://img.shields.io/badge/License-MIT-green.svg)

A compact desktop widget that displays the remaining usage for the locally authenticated **Codex CLI** ChatGPT account.

## Features

- **Capsule window**: A frameless capsule bar (window 300×225) showing the 5-hour and weekly usage windows side by side.
- **Window dragging**: Drag the capsule to reposition it. It automatically snaps to screen edges and corners while avoiding the taskbar, and restores its position after restart.
- **Left side**: Shows the remaining percentage for the 5-hour window with an indigo gradient fill.
- **Right side**: Shows the remaining percentage for the weekly window with a coral-to-rose gradient fill.
- **Click the left side**: Temporarily shows the exact 5-hour reset time, such as `20:17` or `9/21 02:00` when it crosses into another day, then restores the percentage after five seconds.
- **Click the right side**: Temporarily shows the exact weekly reset time, then restores the percentage after five seconds.
- **Settings panel**: Right-click the capsule to open the settings panel, which defaults to 360×380.
  - Drag the panel header to move the window.
  - Drag the lower-left corner to resize the panel freely. The size is saved automatically.
  - **Interface theme**: Light, dark, or system theme.
  - **Language**: Switch instantly between Chinese and English.
  - **Capsule themes**: Choose from Flat, Skeuomorphic, Pixel Game, and Neon Glow styles. Pixel Game uses Pixelify Sans for its numbers; Chinese text keeps the shared UI font.
  - **Per-theme colors**: Customize the capsule background and the left/right progress fills for the selected theme. Each theme keeps its own colors, and the theme previews reflect those custom colors and the current usage values.
  - **Opacity**: Adjust the window opacity in real time.
  - **Text size**: Adjust the usage value font size.
  - **Show usage values**: Hide the percentage values while keeping progress fills visible. Clicking either side still temporarily shows its reset time.
  - **Behavior controls**: Toggle always-on-top and edge snapping, switch between remaining and used percentages, and choose how long reset times stay visible (3, 5, 8, or 10 seconds).
  - **Refresh and retention**: Choose an automatic refresh interval of 1, 2, 5, 10, or 30 minutes, refresh immediately, and retain local history for 7, 30, 90, or 365 days.
  - **Show capsule**: Hide the capsule and restore it later from the tray or with the global shortcut.
  - All settings are saved automatically and restored after restart.
- **Automatic refresh**: Fetches updated usage from the OpenAI backend every five minutes by default; the interval is configurable from 1 to 30 minutes.
- **Usage history**: Opens a dedicated 920×600 **Usage History** window from the Stats tab in the settings panel. It includes two tabs:
  - **Quota history**: A segmented selector for the quota window (weekly) and a mode segmented control (current period, last 7/14 days, last month, browse). Shows KPI cards (current remaining, lowest remaining, sample count, span) and an ECharts line chart of remaining percentage over time.
  - **Token activity**: A range selector (7/30/90 days, 1 year, all) with KPI cards (total, peak, daily average, active days) and a daily token bar chart.
  - Both tabs offer refresh, **Export CSV** (downloads the local quota samples), and a red **Clear history** button. Retention (7/30/90/365 days) is configurable in the settings panel.
- **Usage reset credits**: Shows available reset credits and expiration details, with a two-step confirmation before resetting both weekly and 5-hour limits.
- **Desktop integration**: Includes launch at startup, a system tray, and low-usage notifications. Notifications can be enabled or disabled with 10%, 20%, or 30% thresholds. The tray icon can use the app logo or dynamic dual usage rings. On macOS, Capsule Meter runs as a menu-bar utility without a Dock icon, opens a frosted-glass preferences window, and does not show a separate floating capsule. Close the window to keep the app in the menu bar; use the selector at the top of the window to switch the menu-bar icon.
- **Consumption speed**: The dots outside the capsule show recent consumption speed: gray = assessing (not enough data), green = slow, blue = normal, orange = fast, and red = critical. The left dot is for the 5-hour window and the right dot is for the weekly window. Hover over the corresponding capsule side to see the estimate.
- **Custom shortcut**: `Ctrl+Shift+U` shows or hides the capsule by default; on macOS it shows or hides the preferences window. A new key combination can be recorded directly in System settings, and the shortcut can be disabled.
- **Account information**: Shows the current Codex account below the plan name. The value is read only from local Codex authentication data.
- **Maintenance tools**: Re-authenticate, run Codex diagnostics, and check Gitee Releases for updates.

## Technology

- **Frontend**: Vue 3, TypeScript, and Vite
- **Desktop framework**: Tauri 2 and Rust
- **Data source**: ChatGPT credentials stored in the local `~/.codex/auth.json` file. Usage is fetched from `https://chatgpt.com/backend-api/wham/usage`, the same source used by Codex CLI `/status`.

## Data Details

| Data | Source |
|---|---|
| Remaining 5-hour percentage | `primary_window.used_percent`; remaining = 100 − used |
| Remaining weekly percentage | `secondary_window.used_percent`; remaining = 100 − used |
| Reset time | `reset_at` Unix timestamp |

- Access, refresh, and ID tokens are used only by the local Rust backend and are **never** exposed to the frontend or a third party. Only the account email extracted locally from the ID token is returned for display in the settings panel.
- If authentication expires with HTTP 401, run `codex login` or open the Codex app once to refresh the session.
- Edge snapping uses the Windows monitor work area, automatically avoids the taskbar, and uses a threshold of 24 logical pixels. macOS uses a centered settings window instead of restoring a floating capsule position.

## Common Commands

```bash
# Install dependencies
pnpm install

# Development mode with hot reload
pnpm tauri dev

# Build a universal macOS .app (Apple Silicon + Intel)
pnpm build:macos

# Build only for Apple Silicon
pnpm build:macos:arm64

# Build a Windows NSIS installer and automatically increment the version
pnpm build:patch   # Patch release: 1.0.1 → 1.0.2
pnpm build:minor   # Minor release: 1.0.1 → 1.1.0
pnpm build:major   # Major release: 1.0.1 → 2.0.0

# Build with the current version (does not increment the version number)
pnpm tauri build

# Increment the version without building
node scripts/bump-version.js patch   # Or minor / major
```

`pnpm tauri build` uses the version already configured in `package.json` and `src-tauri/tauri.conf.json`; it does not automatically change either version.

## Release and Build Outputs

The Windows x64 NSIS installer is produced at:

```text
Executable:  src-tauri/target/release/capsule-meter.exe
Installer:   src-tauri/target/release/bundle/nsis/CapsuleMeter_<version>_x64-setup.exe
```

The macOS build commands produce these app bundles:

```text
Universal app: src-tauri/target/universal-apple-darwin/release/bundle/macos/CapsuleMeter.app
Apple Silicon: src-tauri/target/aarch64-apple-darwin/release/bundle/macos/CapsuleMeter.app
```

Open `CapsuleMeter.app` to run it. Local macOS bundles use ad-hoc signing. Public distribution outside the App Store needs a Developer ID signature and notarization; an ad-hoc build may require allowing the app in macOS Privacy & Security settings.

Executables, app bundles, and installers are local build artifacts and are not committed to the repository. `pnpm build:macos` uses the current version and does not increment it.

## Project Structure

```text
capsule-meter/
├── src/                    # Vue 3 frontend
│   ├── App.vue             # Capsule window and settings panel
│   ├── components/         # Statistics, reset credits, appearance, and shared UI components
│   ├── composables/        # Theme, locale, history, and global-shortcut logic
│   ├── types/              # Frontend data types
│   └── views/              # Usage history window
├── src-tauri/
│   ├── capabilities/       # Tauri permissions
│   ├── icons/              # Application icons
│   ├── src/
│   │   ├── usage.rs        # Reads authentication and requests usage data
│   │   ├── window_pos.rs   # Window-position persistence
│   │   └── lib.rs          # Application entry point
│   └── tauri.conf.json     # Window configuration (capsule 300×225, frameless, always on top; history 920×600)
```

## Known Limitations

- A local `~/.codex/auth.json` file is required. Usage cannot be loaded until Codex is authenticated.
- macOS builds are ad-hoc signed by default. Configure an Apple Developer ID certificate and notarization credentials before distributing downloads publicly.
- When authentication expires, use **System → Diagnostics & updates** to start `codex login`, then refresh the usage data.
- Update checks query Gitee Releases. The application never downloads or installs updates automatically in the background.
