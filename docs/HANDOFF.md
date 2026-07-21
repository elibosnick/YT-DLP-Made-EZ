# Handoff: what distribution needs to do

The app is code-complete against the spec. This is what has to happen before it can go
out to real users, roughly in order. Nothing here requires touching application logic.

---

## 0. Delete two stray directories before `git init`

The environment this was built in could not delete files, so two bits of cruft are still
sitting in the tree. Remove them before you initialise the repo:

```bash
rm -rf src-tauri/.git                    # stray repo, created by accident
rm -f  vite.config.js.timestamp-*.mjs    # Vite's transient config cache
rm -f  src-tauri/core/Cargo.lock         # only the workspace root's lockfile matters
```

`src-tauri/.git` matters most: a nested `.git` directory will make git treat `src-tauri`
as a separate repository and silently skip all the Rust source when you commit.

`.gitignore` already covers the other two if you miss them.

---

## 1. Placeholders that must be filled in

These are the only things standing between this repo and a working release. Each is
marked with `TODO` or `OWNER/REPO` in the source.

| Where | Placeholder | What to set it to |
| --- | --- | --- |
| `src-tauri/tauri.conf.json` | `identifier: "com.TODO-YOUR-ORG.videodownloader"` | A real reverse-domain id you control. **Must be set before the first public release** — changing it later makes existing installs unable to see updates. |
| `src-tauri/tauri.conf.json` | `plugins.updater.endpoints` → `OWNER/REPO` | Your GitHub org and repo. |
| `src-tauri/tauri.conf.json` | `plugins.updater.pubkey` | Public half of the signing keypair (step 2). |
| `src-tauri/Cargo.toml` | `repository = ".../OWNER/REPO"` | Same repo URL. |
| `README.md` | Product name, if you rename it | The name currently appears as "Video Downloader" per the mockup. |

The product name lives in exactly two places: `productName` and `app.windows[0].title`
in `tauri.conf.json`. Renaming is a two-line change.

---

## 2. Updater signing keys

The in-app updater refuses any bundle that is not signed with the matching key. This is
what stops someone serving a malicious "update".

```bash
npm run tauri signer generate -- -w ~/.tauri/video-downloader.key
```

This prints a public key and writes a private key.

- **Public key** → paste into `plugins.updater.pubkey` in `tauri.conf.json`. Committed.
- **Private key** → GitHub repo secret `TAURI_SIGNING_PRIVATE_KEY`. **Never committed.**
- **Password** → GitHub repo secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

Back the private key up somewhere durable. If you lose it, you cannot ship updates to
anyone who already installed the app — they would all have to reinstall manually.

---

## 3. First release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The `build-release.yml` workflow builds all four targets and opens a **draft** release
with the installers and `latest.json` attached. Review it, then publish.

It is a draft rather than an auto-publish deliberately: the first release is worth
eyeballing before the updater starts pointing users at it.

---

## 4. macOS code signing — the one real gap

**Current state: unsigned.** The build works and the `.dmg` installs, but on first open
macOS shows *"Video Downloader can't be opened because Apple cannot check it for
malicious software."* The user has to right-click → Open, or visit System Settings →
Privacy & Security.

For this app's audience, that is close to fatal. A grandma who sees that message will
assume the app is broken or dangerous, and stop.

To fix it you need:

1. An **Apple Developer Program** membership (~$99/year).
2. A **Developer ID Application** certificate.
3. Notarization credentials (an app-specific password, or an App Store Connect API key).

Then uncomment the `APPLE_*` block in `.github/workflows/build-release.yml` and add the
corresponding repo secrets. Tauri's action handles signing and notarization from there —
no code changes.

**Windows** has a milder version of the same problem: SmartScreen warns on unsigned
installers until enough people install it to build reputation. An EV code-signing
certificate removes the warning immediately. Less urgent than macOS, but worth costing.

---

## 5. Things I would test on real hardware first

The logic is unit tested (40 Rust + 33 frontend), but these can only be verified on a
real machine with a real network:

- [ ] First run on a clean machine — the yt-dlp + ffmpeg fetch is the least-tested path,
      and it is the one every single user hits.
- [ ] First run **offline** — should show a friendly message, not hang or crash.
- [ ] All three format presets against YouTube, TikTok, and Instagram.
- [ ] A long video (>1GB) — confirms progress stays responsive and nothing times out.
- [ ] A video with a non-ASCII title (Japanese, Arabic, emoji) — filename handling.
- [ ] A video that is private / deleted / age-restricted — confirms the friendly errors
      fire and the "Try again?" button appears only where retrying could help.
- [ ] Pull the network cable mid-download.
- [ ] The updater end-to-end: install v0.1.0, publish v0.1.1, confirm the prompt appears
      and the install succeeds. **This cannot be tested before the first release exists**,
      and it is the single most important thing to verify — a broken updater cannot be
      fixed by shipping an update.
- [ ] Windows: confirm no console window flashes on launch or during a download.

---

## 6. Known limitations, deliberate

- **ffprobe is not downloaded**, only ffmpeg. yt-dlp prefers ffprobe for codec detection
  but falls back to `ffmpeg -i`, and both our ffmpeg-dependent paths work without it.
  Adding it would roughly double the ~80MB first-run download. If codec detection ever
  causes trouble, adding it is a five-line change in `ytdlp.rs`.
- **No download cancellation.** Not in the spec. The process is killed if the app quits
  (`kill_on_drop`), so nothing is orphaned, but there is no in-UI stop button.
- **No proxy support.** Users behind a corporate proxy will fail at first-run setup.
  Out of scope for the target audience, but worth knowing.
- **Windows 7/8** are on the spec's test checklist. Tauri v2 requires WebView2, which
  Microsoft supports back to Windows 7 — but Windows 7 is long out of support and I would
  not assume it works without testing it. Windows 10/11 are safe.

---

## 7. Upstream fragility — the thing most likely to break in six months

The app downloads yt-dlp and ffmpeg from URLs owned by other projects. If either renames
an asset, **new installs break at first run** while existing installs carry on fine — so
it fails silently, for exactly the users you cannot afford to lose.

`scripts/verify-binary-urls.sh` checks every URL, and `check-binary-urls.yml` runs it
weekly. If that job starts failing, fix the URL table in `src-tauri/src/ytdlp.rs` and cut
a release. Keep the workflow enabled; it is cheap and it is the early warning.

(This is not hypothetical — the Windows ffmpeg asset name in the original draft of this
app was wrong, and this script is what caught it.)
