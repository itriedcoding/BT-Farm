# BT Farm

A Windows desktop control plane for cloud Linux infrastructure and **real** Android devices:
provision workspaces on five providers, drive phones over ADB, watch their screens live, and run
automated workflows on a schedule.

Built with Rust (core engine + Tauri v2 shell), React + TypeScript (UI) and SQLite (local store).

> **No mocks.** Every number, screen frame and log line in the interface comes from a provider API, an
> `adb` process or the local database. There are no seeded instances, no simulated devices, no
> placeholder buttons, and no hard-coded statistics.

---

## Download and install

1. Open the [Releases](https://github.com/itriedcoding/BT-Farm/releases) page and download
   `BT Farm_1.0.0_x64-setup.exe`.
2. Run it. The installer is **per-user** (no administrator prompt) and installs to
   `%LOCALAPPDATA%\BT Farm`.
3. Launch **BT Farm** from the Start menu. Uninstall through Settings → Apps at any time.

Requirements:

| Requirement | Why | Needed for |
| --- | --- | --- |
| Windows 10 1809+ x64 | Tauri/WebView2 runtime | everything |
| WebView2 Runtime | renders the UI | installed by setup if missing |
| Android platform-tools (`adb.exe`) | device transport | Android device screens |
| `boxd` CLI | boxd has no stable public HTTP API | the boxd provider only |
| ffmpeg (optional) | encodes PNG recordings to MP4 | recording |

The app starts without any of the optional tools and tells you exactly what is missing on the
Devices and Settings screens.

---

## What each provider can actually do

None of these five providers boots an Android emulator: they rent Linux VMs/containers without KVM
or GPU acceleration. BT Farm therefore never labels an instance a "phone". Android devices are
attached separately over ADB, and the app says so in the UI.

| Provider | Transport | Provision | Start/stop | Exec | Metrics | Android VM |
| --- | --- | --- | --- | --- | --- | --- |
| E2B | REST `https://api.e2b.app` | yes | pause / resume / kill | no ¹ | yes (`/sandboxes/metrics`) | no |
| Boat | REST `https://boat.dev/api/v1` | yes | yes | yes (`/commands`) | no | no |
| boxd | official CLI `boxd … --json` | yes | delete only ² | yes (`machine exec`) | no | no |
| Daytona | REST `https://app.daytona.io/api` | yes | start / stop / delete | no | no | no |
| Freestyle | REST `https://api.freestyle.sh` (v5) | yes | start / stop / delete | yes | no | no |

¹ E2B command execution runs through `envd` inside the sandbox, which is not part of the platform
REST API; BT Farm reports `exec` as unsupported instead of faking it.
² boxd exposes no stop verb through its CLI; instances are deleted.

The same matrix is rendered live in **Infrastructure** and **Settings**, read from the Rust core, and
each chip carries the provider's own limitation note.

### Credentials

| Provider | Credential | Notes |
| --- | --- | --- |
| E2B | API key | sent as `X-API-Key` |
| Boat | API token | `Authorization: Bearer` |
| boxd | token | exported to the CLI as `BOXD_TOKEN` |
| Daytona | API key | `Authorization: Bearer` |
| Freestyle | API key | `Authorization: Bearer` |

Keys are entered under **Infrastructure → Configure**, verified with **Test** (a real call to the
provider's account endpoint) and sealed with **Windows DPAPI** under your account. They are never
written to the log, never sent to the webview, and error responses are redacted before they reach
the UI.

---

## Android devices

Android is handled entirely through the real `adb` binary:

- `adb devices -l` for enumeration, properties cached per serial
- `adb -s <serial> shell` for commands, `input tap|swipe|text|keyevent` for interaction
- `adb exec-out screencap -p` for frame capture
- `adb install -r -g`, `pm list packages -3`, `am force-stop`, `monkey` for app management
- `adb logcat -d -t <n>` for diagnostics
- `adb connect host:port` / `disconnect` for network devices (cloud phone hosts, Redroid/Waydroid
  hosts, Android x86 VMs, emulators)

**Live screens** (`Live screens`) poll `screencap` at 1–15 fps per device, push base64 PNG frames
over the Tauri event bridge, and accept mouse input: click to tap, drag to swipe, plus Back / Home /
Recents / text keys. Recording writes numbered PNG frames into the app data directory and encodes an
MP4 with ffmpeg when it is available.

---

## Automation

Workflows are ordered step lists with per-step retries, timeouts and `continue on error`:

- **Android**: tap, swipe, type, key event, launch/stop app, install APK, shell command, wait for
  activity, screenshot
- **WebDriver (Selenium)**: navigate, click, type, wait for selector, assert text, screenshot —
  spoken directly over the W3C WebDriver protocol, so it works with a local Selenium server, a Grid
  inside a provisioned VM, or any W3C-compatible endpoint
- **Integration**: HTTP request with headers/body and expected status
- **Flow**: wait, note

Runs stream their log lines to the UI as they execute and are persisted with status, trigger,
duration and error. **Scheduler** fires workflows every N minutes or daily; due runs are computed
from stored timestamps and dispatched by the background loop while the app is open.

**Publishing** uses official APIs only: Mastodon's documented `POST /api/v1/statuses` with an access
token you create in your instance, or a templated HTTP request against any official endpoint you
already have credentials for (for platforms that require OAuth, obtain the token yourself through
their documented flow and paste it here). No account creation, no CAPTCHA solving, no rate-limit or
ban evasion, no spam tooling — the app refuses to grow in that direction.

---

## Security and privacy

- **Credentials**: Windows DPAPI (`CryptProtectData`, per-user) via `windows-sys`; on non-Windows
  development builds a 0600 file is used and the backend reports that weaker mode in Settings.
- **No secrets in logs**: provider clients redact `Authorization`, `X-API-Key`, tokens and
  token-shaped strings before a message is logged or returned.
- **Local-first**: the SQLite database, logs, screenshots and recordings live under
  `%APPDATA%\com.btfarm.desktop`. Nothing is uploaded anywhere except to the provider you call.
- **Least privilege**: no elevated install, no Windows service, no background daemon after exit.
  Streams and runs are stopped when the app closes, and stale runs are marked failed on next start.
- **Process isolation**: every external action is an explicit `adb`, `boxd` or `ffmpeg` process with
  a timeout; commands are argument vectors, never shell strings, except where `adb shell` requires
  a single command string.

---

## Architecture

```
crates/farm-core        platform-independent engine (compiles and tests on Linux/macOS/Windows)
  providers/            e2b, boat, boxd, daytona, freestyle clients over one trait
  android/              adb wrapper, device enumeration, screencap stream manager
  automation/           workflow runner + W3C WebDriver client
  store.rs              SQLite schema, CRUD, due-schedule queries
  secrets.rs            DPAPI (Windows) / 0600 file (fallback) secret store
  http.rs               retrying HTTP client with redaction and error extraction
  model.rs              all shared types (serde, snake_case enums)
src-tauri/              thin Tauri v2 shell: 56 commands, event bridge, file dialogs
src/                    React + TypeScript UI (no UI framework, hand-written design system)
```

The webview talks to the core only through Tauri commands; real-time updates arrive on the
`farm-event` channel. The UI contains no business logic and no data of its own — when the backend is
unreachable it renders an explicit offline banner rather than fake content.

### Build from source

```bash
npm ci
cargo test -p farm-core          # 28 engine tests, no credentials required
npm run build                    # tsc --noEmit + vite build
npm run tauri build              # requires Windows + WebView2 + MSVC toolchain
```

`npm run tauri build` produces `src-tauri/target/release/bundle/nsis/BT Farm_1.0.0_x64-setup.exe`.
Optimised profile: LTO, `opt-level = "s"`, single codegen unit, stripped symbols, panic=abort.

### Releases

`.github/workflows/release.yml` runs on every push to `main`:

1. **verify** (Ubuntu) — `npm ci`, `npm run build`, `npm test`, `cargo test -p farm-core`
2. **windows-installer** (Windows) — builds the NSIS installer with `tauri-action` and uploads it to
   the `v1.0.0` GitHub release

If the verify job fails, nothing is published.

---

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| "adb binary not found" | Install Android platform-tools and set the path in **Settings → adb executable** |
| Device shows `unauthorized` | Accept the USB debugging prompt on the phone |
| Screen stays black | Wake the device and confirm `adb exec-out screencap -p` works in a terminal |
| Provider says 401 | Re-enter the key under **Infrastructure → Configure** and press **Test** |
| boxd missing | Install the official `boxd` CLI and set its path in Settings |
| Recording has no MP4 | Install ffmpeg or set its path; the PNG frames are kept either way |

Logs live in `%APPDATA%\com.btfarm.desktop\logs` and are readable in **Logs**.

---

## Scope and compliance

BT Farm manages infrastructure you are entitled to use and devices you own or are authorised to
control. It intentionally does not implement CAPTCHA bypasses, platform security evasion, spam
automation, fake engagement, or circumvention of account-creation restrictions. Publishing is
limited to official APIs with credentials you provide.

## License

MIT.
