# YT-DLP Made EZ

A dead-simple desktop app for saving videos from YouTube, TikTok, Instagram, Twitch, and
[a thousand other sites](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md).

Paste a link. Pick a format. Click Download. The file lands in your Downloads folder.
That's the whole app.

**Everything happens on your computer.** No account, no ads, no tracking, no telemetry.

---

## For people who just want to use it

Download the file for your computer from the [Releases page](../../releases):

| Your computer | File to download |
| --- | --- |
| Windows | the `.msi` file |
| Mac with Apple Silicon (M1/M2/M3/M4) | the `aarch64.dmg` file |
| Mac with Intel | the `x64.dmg` file |
| Linux | the `.AppImage` file |

Open it, and the app installs itself.

**The first time you open the app**, it spends a minute downloading two helper programs
it needs (`yt-dlp` and `ffmpeg`). You need an internet connection for that first launch.
After that it starts instantly.

### Using it

1. Copy the web address of the video you want.
2. Paste it into the box.
3. Choose what you want:
   - **Best Quality (Video + Audio)** — an ordinary MP4 video. Pick this one.
   - **Audio Only (MP3)** — just the sound, as a music file.
   - **Best Available** — the highest quality there is. Can produce very large files.
4. Click **Download**.

Your file appears in your Downloads folder.

### If something goes wrong

The app explains problems in plain language. Some videos simply cannot be downloaded —
private videos, age-restricted videos, and videos the site has locked down. That is a
limit of the site, not a bug in the app.

---

## Legal

This tool is for downloading videos you have the right to download. That includes:

- Videos you created
- Videos under a Creative Commons license
- Videos in the public domain
- Videos where the creator allows downloads

Respecting copyright and platform terms of service is your responsibility. The authors
assume no liability for misuse.

## Privacy

This app runs entirely on your computer. No personal data is sent to anyone.

The only network requests it makes are:

1. Downloading the video you asked for, directly from the site it is on.
2. Fetching `yt-dlp` and `ffmpeg` from GitHub on first run, and refreshing `yt-dlp`
   about once a day.
3. Asking GitHub whether a newer version of this app exists.

No tracking, no ads, no telemetry, no analytics.

---

## For developers

### Building from source

You will need [Node 20+](https://nodejs.org), [Rust](https://rustup.rs), and the
[Tauri system dependencies](https://tauri.app/start/prerequisites/) for your platform.

On Ubuntu/Debian that is:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

Then:

```bash
npm install
npm run tauri dev      # run in development, with hot reload
npm run tauri build    # produce an installer for your platform
```

### Project layout

```
video-downloader/
├── src/                      # React frontend
│   ├── App.jsx               # state machine: setup → ready → downloading → done
│   ├── App.css               # all styling; light/dark via prefers-color-scheme
│   ├── format.js             # human-readable sizes, speeds, and time estimates
│   └── components/
│       ├── DownloadForm.jsx
│       ├── FormatSelector.jsx
│       ├── ProgressBar.jsx
│       ├── StatusMessage.jsx
│       └── UpdateBanner.jsx
├── src-tauri/
│   ├── core/                 # ← pure logic, no Tauri dependency, heavily tested
│   │   └── src/
│   │       ├── args.rs       # format presets → yt-dlp command line
│   │       ├── error.rs      # yt-dlp stderr → friendly user-facing messages
│   │       ├── progress.rs   # parsing yt-dlp's output stream
│   │       └── urls.rs       # URL validation
│   └── src/                  # Tauri adapter layer
│       ├── main.rs           # app setup, background tasks
│       ├── commands.rs       # the commands the frontend calls
│       ├── ytdlp.rs          # fetching and updating yt-dlp + ffmpeg
│       ├── updater.rs        # app self-update, yt-dlp refresh schedule
│       └── process.rs        # spawn helper (suppresses console windows on Windows)
├── scripts/
│   ├── generate-icons.py     # regenerates every icon size from one definition
│   └── verify-binary-urls.sh # checks upstream asset URLs still exist
└── .github/workflows/
    ├── build-release.yml     # tag → installers for all platforms
    └── check-binary-urls.yml # weekly upstream URL check
```

**Note:** `index.html` is at the project root rather than inside `src/`. That is Vite's
convention and where its build expects it.

### Why the `core` crate exists

The logic most likely to break — URL validation, yt-dlp's argument list, parsing its
progress output, translating its errors — lives in `src-tauri/core`, which depends on
neither Tauri nor any GUI toolkit.

That means:

```bash
cargo test --manifest-path src-tauri/core/Cargo.toml
```

runs in about a second on any machine, with no `webkit2gtk` and no system packages. It
is the fast inner loop, and it is where you should add a test when you fix a bug.

### Running the tests

```bash
npm run test                                          # frontend (33 tests)
cargo test --manifest-path src-tauri/core/Cargo.toml   # core logic (40 tests)
bash scripts/verify-binary-urls.sh                     # upstream URLs still valid
```

### A few things worth knowing before you change the code

- **`--no-playlist` is load-bearing.** Remove it and a link with a `&list=` parameter
  downloads the entire playlist. There is a test guarding this.
- **`--print` implies `--simulate`.** `--no-simulate` is what stops the app cheerfully
  reporting success while downloading nothing. Also tested.
- **stderr must be drained concurrently with stdout.** If you refactor the download loop
  to read only stdout, a chatty download will fill the OS pipe buffer and the app will
  hang with no error.
- **ffmpeg is not optional.** Two of the three format presets need it.
- **Error messages are user-facing copy.** `core/src/error.rs` has a test asserting that
  no message leaks jargon like "yt-dlp" or "HTTP error". Keep it that way.

### Contributing

Issues and pull requests are welcome. Please make sure `npm run test` and the core crate
tests pass. If you are fixing a bug, add a test that would have caught it.

Design principle to hold the line on: this app is for people who find software
intimidating. Every added button, option, or configuration screen makes it worse for
them. The [non-features list](#) is deliberate — playlists, custom formats, batch
downloads, and a settings panel were all considered and left out on purpose.

## License

MIT — see [LICENSE](LICENSE).

`yt-dlp` is [Unlicense](https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE).
`ffmpeg` static builds are LGPL/GPL licensed; see the upstream
[ffmpeg-static](https://github.com/eugeneware/ffmpeg-static) project. Both are downloaded
at runtime rather than bundled, and neither is modified.
