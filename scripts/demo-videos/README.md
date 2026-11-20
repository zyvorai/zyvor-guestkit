# GuestKit demo-video pipeline

Three demo videos, two different capture techniques, same ffmpeg
composite pipeline (title cards + caption overlays) as
`hypersdk-/scripts/demo-videos/` and `hyper2kvm-/scripts/demo-videos/`.

## Requirements

- `playwright` (symlinked `node_modules` — see below) with the `chrome` channel installed, for the web-dashboard recordings
- [`vhs`](https://github.com/charmbracelet/vhs) (`brew install vhs`), for the CLI/TUI recording
- `ffmpeg` (needs `overlay`/`fade`/`concat` filters — captions are pre-rendered PNGs, no `drawtext`/`libfreetype` required)
- `python3` (small float arithmetic in the build scripts)
- A GuestKit CLI/TUI install on a real Linux host (NBD/loop mounts aren't available on macOS) — see `scripts/deploy-remote.sh`
- A GuestKit web stack (`zyvor-ui`/`zyvor-api`/`guestkit-worker`) on k3s — see `deploy/scripts/deploy-remote-k3s.sh`

`node_modules` here is a symlink to a sibling project's `dashboard-react/node_modules`
(gitignored, not committed) — point it at any checkout with `playwright` installed.

## 1. CLI / TUI video (`rec-inspect.tape`, `rec-doctor.tape`, `rec-migrate.tape`, `rec-tui.tape`)

Each command gets its own short VHS tape (a single long SSH session risked
dropping mid-recording) driving a real `guestkit` binary over SSH against a
real Ubuntu cloud image.

```bash
node render-cards.mjs
vhs rec-inspect.tape   # -> raw/inspect-raw.mp4
vhs rec-doctor.tape    # -> raw/doctor-raw.mp4
vhs rec-migrate.tape   # -> raw/migrate-raw.mp4
vhs rec-tui.tape       # -> raw/tui-raw.mp4
./build.sh             # -> out/guestkit-cli-demo.mp4
```

Edit the `ssh sus@<host>` line and disk path in each `.tape` file for a
different target host/image. VHS compresses static/idle frames on its own,
so `extract_clip` windows in `build.sh` are tuned against the actual
recording, not the tape's nominal `Sleep` durations — re-check them (ffmpeg
`-ss`/`-frames:v 1` frame dumps are the fastest way) after any tape edit.

## 2. Web dashboard videos (`rec-web-tour.mjs`, `rec-web-tutorial.mjs`)

Playwright recordings of the `zyvor-ui` web dashboard: browsing a live
KubeVirt cluster, selecting a real VM, and running Fingerprint/Boot Doctor
from the browser.

```bash
node render-cards-web.mjs
export GK_WEB_URL=http://<host>:30081/
node rec-web-tour.mjs      # -> raw/web-tour/*.webm
node rec-web-tutorial.mjs  # -> raw/web-tutorial/*.webm
cp raw/web-tour/*.webm web-tour-raw.webm
cp raw/web-tutorial/*.webm web-tutorial-raw.webm
./build-web.sh             # -> out/guestkit-web-tour.mp4, out/guestkit-web-tutorial.mp4
```

Use `#dockInspect`/`#dockDoctor`/etc. (the bottom-dock button IDs) rather
than text locators for the pipeline-stage labels at the top of the page —
those are inert "locked" placeholders for the disk-upload flow and are not
the actual action buttons when browsing a cluster VM.

## Publishing

Not committed here (binary, and per-run): upload the built MP4s to YouTube
and copy them locally, following the same convention as prior demo videos
(public visibility, title cards + captions, copy under `~/Desktop/`).

Published videos (thumbnails served by YouTube — see the top-level README's
[demo table](../../README.md#-see-it-in-action), no local copies needed):

| Video | YouTube |
|---|---|
| Machina × GuestKit live guest-agent UX | https://youtu.be/LYoqOye3P3I |
| CLI/TUI demo | https://youtu.be/lLEBQoFceIs |
| Web dashboard tour | https://youtu.be/usQX2rQIFM8 |
| Web dashboard tutorial | https://youtu.be/icTLVko588A |

The Machina live-agent reel is recorded from the Machina platform
(`machina/scripts/demo-videos/seg-gk-agent.mjs` → `build-guestkit.sh`) against
a real KVM guest with GuestKit/QGA; publish under
`~/Desktop/PacketWolf-Demo-Videos/machina-guestkit/`.
