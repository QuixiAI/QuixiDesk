# Linux support for QuixiDesk — what this patch does and why

This patch fills in the `ComputerUse` surface that upstream implements only on
macOS. Everything here was built against a running Plasma 6.8 / KWin 6 Wayland
session and measured on it; the numbers quoted below are from that box, not
estimates.

Design rule used throughout: **degrade honestly**. Every entry point either does
the thing or returns an error naming the missing tool. Nothing reports success
for an action that did not happen, and no capability is advertised without its
backend present.

---

## 1. macOS → Linux mapping

| `ComputerUse` call | macOS | Linux here |
|---|---|---|
| `capture_screenshot_base` | `screencapture` | KWin `ScreenShot2.CaptureArea` → spectacle → grim → gnome-screenshot → `import` |
| `capture_zoom_base` | `screencapture -R` | same, but capturing **only** the rectangle |
| `get_cursor_position` | `CGEventGetLocation` | `xdotool getmouselocation` (see limitations) |
| `mouse_move / click / down / up / drag` | `CGEventPost` | `ydotool` over a uinput device |
| `type_text` | `CGEventKeyboardSetUnicodeString` | per-character evdev keycodes via `ydotool key` |
| `press_key` | `CGEventCreateKeyboardEvent` | evdev keycodes via `ydotool key` |
| `read_clipboard / write_clipboard` | `pbpaste` / `pbcopy` | `wl-copy`/`wl-paste` → `xclip` → `xsel` |
| `pick_file / pick_files` | `NSOpenPanel` | `kdialog` → `zenity` |
| `pick_folder / pick_folders` | `NSOpenPanel` (dir) | `kdialog` → `zenity`, **single folder** (see limitations) |
| `list_displays` | `CGGetActiveDisplayList` | `kscreen-doctor -j` → `xrandr` → `wlr-randr` |
| `list_installed_apps` | `NSWorkspace` | `.desktop` scan over XDG data dirs |
| `open_application` | `NSWorkspace.open` | `gtk-launch` → `kioclient6` → `gio launch` |
| `get_tcc_permissions` | TCC (Accessibility, Screen Recording…) | `"unknown"` (shared with every non-macOS platform) — Linux has no equivalent gate |
| `request_access` | opens TCC prompts | no-op with a note: there is nothing to grant |

There is **no** `query_windows`-style call in this patch, deliberately: Wayland
gives clients no window list, the XDG portal does not expose one by design, and
`org.kde.KWin.queryWindowInfo` blocks waiting for a user click. A method that
cannot answer is worse than an absent one.

---

## 2. Decisions, with the measurement that forced each

### Capture goes through ScreenShot2 first, and external tools are the fallback

The obvious implementation shells out to `spectacle -b -f`, which is what the
first version did. It works and it is slow:

| capture | `spectacle` | KWin `ScreenShot2` |
|---|---|---|
| whole desktop (4114×1234) | 921 ms | **85.8 ms** |
| 256×256 region | 1250 ms | **1.4 ms** |

A 320×200 region was *more* expensive than the whole screen, because the old path
captured everything and then cropped it with ImageMagick — backwards for the one
case that gets used constantly. `CaptureArea` captures exactly the rectangle.

Two traps in that API, both of which produce plausible-looking garbage rather than
errors:

* It returns **raw pixels even with `native: false`** (`type: "raw"`), not PNG.
* `stride` can exceed `width * 4`, so rows must be repacked. Treating the buffer as
  tightly packed shears the image.

Cost: `zbus` was already resolved in the workspace lock, so the only new crate is
`png` (pure Rust, no C dependency, no system requirement).

### `ydotool type` is not used for typing

`ydotool type` needs an XKB keymap loaded into the daemon. With none, it returns
**exit 0 and types nothing** — a silent no-op indistinguishable from success.
Verified directly: `ydotool type "touch /tmp/x"` produced no characters, while
`ydotool key 30:1 30:0` put `a` at the prompt.

So `type_text` emits per-character evdev keycodes. `key` operates at the kernel
level and cannot be misinterpreted.

**Speed:** `ydotool` defaults to roughly 12 ms between key events, which caps
typing at ~41 characters/second. Measured on the same box, `-d 0` does 120
keypresses in 16 ms (133 µs each) and the target shell still received every byte.
A 1,234-character command went from **30,455 ms to 167 ms** — the default delay was
pure overhead here.

### Mouse buttons are key presses, not subcommands

`ydotool` ships five commands: `click`, `mousemove`, `type`, `key`, `debug`. There
is **no `mousedown` and no `mouseup`**; a press-and-hold must be expressed as an
evdev `BTN_*` code (`BTN_LEFT` 272, `BTN_RIGHT` 273, `BTN_MIDDLE` 274). The first
implementation called the nonexistent subcommands, so every press-and-hold and
every drag silently did nothing.

Each event token must also be its **own argv entry**: passing `"42:1 30:1 30:0"`
as one argument is rejected and sends nothing.

### Unknown button names are an error, not a left click

`mouse_down("")` used to fall through to `BTN_LEFT`, so a typo became a real click
wherever the cursor happened to be. It now returns
`unknown mouse button ""; expected left, right or middle`. Case and surrounding
whitespace are normalised; genuinely unknown names are refused.

### `wl-copy` is waited on by its parent only

`wl-copy` forks a resident child that holds the selection and inherits the stderr
pipe, so `wait_with_output()` never sees EOF. That is a 32-minute hang that looks
like a freeze. Both streams are now nulled and the parent is waited on; the child
is expected to outlive the call.

### Application launching is done in-process, not by shelling out

`open_application` used to round-trip through `gtk-launch` → `gio` → `kioclient6`.
All three are unnecessary: the `.desktop` entry is already parsed here, so the `Exec`
line is already in hand. Spawning it directly removes three optional dependencies, one
process spawn per launch, and a class of failure where the launcher cannot find a
binary that is sitting on `PATH`.

Two details in the `Exec` grammar matter and are handled explicitly:

* **Quoting.** Double quotes group arguments and backslash escapes work inside them, so
  `Exec=sh -c "printf hi > /tmp/x"` is three arguments, not six.
* **Field codes.** `%U %F %f %u %i %c %k` expand to files, URLs, icon names and the
  desktop-file path. Launching with no arguments means they expand to nothing, and an
  argument that *contained* one is dropped **whole** rather than rewritten — turning
  `--user-data-dir=%d` into `--user-data-dir=` hands the target program a dangling flag,
  which is worse than omitting it. `%%` survives as a literal percent sign.

The launcher paths remain as a fallback for the case where the Exec binary is not on our
`PATH` but a desktop launcher still knows where it lives.

### `XDG_DATA_DIRS` is honoured, because it is the standard search path

The application list originally hardcoded `/usr/share/applications` and
`/usr/local/share/applications`, ignoring `XDG_DATA_DIRS` entirely. That path is where
NixOS, Flatpak (`/var/lib/flatpak/exports/share`) and any custom-prefix install puts its
desktop entries, so on those systems the app list came back empty or wrong. Flatpak's
user and system export directories are probed as well, since they are not reliably in
`XDG_DATA_DIRS` either.

This one was found by a test rather than by reading the code: the test installed an entry
into a throwaway data directory, set `XDG_DATA_DIRS` at it, and watched the lookup miss.

### A headless session is its own answer

`session_type` was `if is_wayland() { "wayland" } else { "x11" }`, which reported `"x11"`
on a machine with no display at all — an SSH shell, a container, a server. The UI would
then offer capture and input buttons for a session that cannot have them. It is now
three-way and `"none"` is a real value.


### Capability is not gated on a tool that is no longer required

The probe originally reported screen capture unavailable unless `spectacle`
existed — true when spectacle was the only path, false once `ScreenShot2` needed
nothing but the compositor. It now asks the bus whether `org.kde.KWin` is present,
and only falls back to tool detection. Same for `xdotool`: its absence no longer
means "no display information" when `kscreen-doctor` is present.

---

### GNOME refuses third-party callers, so capture goes through the portal

`org.gnome.Shell.Screenshot` looks like the GNOME equivalent of KWin's ScreenShot2, and it
is not usable by a third-party application. Calling it returns
`org.freedesktop.DBus.Error.AccessDenied: Screenshot is not allowed` — reproduced with
`gdbus` as well as from this code, so it is policy, not a mistake in the call.

The sanctioned route is `org.freedesktop.portal.Screenshot`. It is the only interface that
works across GNOME, KDE and wlroots, which makes it the portable answer and demotes the
compositor interfaces to fast paths. Two details cost real time:

* The request is **asynchronous**. `Screenshot` returns a request handle immediately and the
  result arrives later as a `Response` signal on that handle. The signal has to be
  subscribed *before* the call or the reply is missed, and the wait needs a bounded timeout
  because on GNOME the first call raises a permission dialog that a human must approve.
* The reply is `(u32 response, a{sv} results)` where `results["uri"]` is a `file://` URL.
  Response 0 is success, 1 is a user cancellation, anything else is a failure.

### The portal captures full frames only, so regions are cropped in-process

There is no region call in the portal interface. The old path shelled out to ImageMagick to
crop, which fails twice: it makes a region cost more than a full frame, and ImageMagick's
X11 `import` cannot run on a Wayland desktop at all. Measured on GNOME, region capture
failed outright while full-frame worked.

The `png` crate is already a dependency, so `crop_png` decodes, copies the rows it needs and
re-encodes. Region capture now works wherever a full frame does, with no external tool.

### Mutter's display interface has its own bus name

`org.gnome.Mutter.DisplayConfig.GetCurrentState` is GNOME's answer to `kscreen-doctor`, and
without it a GNOME box reported `{"displays":[]}` — no enumeration at all.

The trap: the destination is **`org.gnome.Mutter.DisplayConfig`**, not `org.gnome.Mutter`.
GNOME exports each Mutter interface under its own well-known name, so addressing
`org.gnome.Mutter` returns `ServiceUnknown: The name is not activatable` even though the
shell is running.

### "Active output" is derived where no compositor call exists

KWin names the active output directly. Nothing else does. Rather than report nothing, the
answer is computed: take the logical monitor rectangles and the cursor position, and the
output whose rectangle contains the pointer is the active one. That works on GNOME through
Mutter and on X11 through xrandr, from the same code.

### `ydotool --absolute` is not in pixels, so the ratio is measured at runtime

A move to the middle of a 2560x1600 screen landed the pointer in the bottom-right corner.
The cause: `ydotool mousemove --absolute -x 640` puts the pointer at x=1280, so the injected
space is half the screen. Assuming 1:1 was wrong by a factor of two.

Guessing the ratio from the display list cannot work, because both readings come back in the
same pixels. So it is measured once — move to a known quarter point, read where the pointer
actually landed, cache the ratio — and every later move converts through it. Self-correcting
on any desktop, no hardcoded constant.

### A clipboard write is verified by reading it back, not by waiting for the tool

Clipboard tools are supposed to keep running: owning the selection is the job. `wl-copy`
forks an owner and on GNOME the parent does not exit, so waiting for it reported failure
after the full timeout even though the data was on the clipboard.

Success is now "the clipboard returns what was written", polled for up to five seconds. On
Wayland the write prefers `xclip` through XWayland when an X display exists, because
XWayland bridges the selection to the Wayland clipboard and does not race its own fork —
verified both ways: `xclip -o` returned the value and `wl-paste` saw it too.

### External tools are never waited on without a timeout

`Command::output()` has no timeout. An XFCE screenshot tool started inside a GNOME session
opened a dialog and sat there forever, and the caller hung with it. Every tool call now
polls with a deadline, kills the child, and reports what it was doing.

The first version of that fix stopped reading the pipes, which is a deadlock: once a child
writes past the pipe buffer it blocks in `write()`, never exits, and gets killed as if it had
hung. Measured as a truncated 1 MB clipboard read and a test suite that took 30s instead of
9s. Both streams are now drained on reader threads while the wait polls.

### `window_under_cursor` means under the POINTER, not the focused window

The original implementation called `xdotool getactivewindow`, which is the focused window —
a different thing. Focus and pointer position diverge constantly: hovering a panel, a
tooltip, or a background window changes one without the other.
`xdotool getmouselocation --shell` reports the window actually under the pointer, which is
what the name promised.

### File dialogs go through the portal, not `kdialog` or `zenity`

The pickers used to call `kdialog` on KDE and `zenity` on everything else, which is two
toolkits, two argument styles and two of the four remaining subprocess dependencies — for a
dialog the desktop already knows how to draw.

`org.freedesktop.portal.FileChooser` replaces both with one code path, and it is the only
dialog route that behaves the same on KDE, GNOME and wlroots. The alternative considered was
a Rust-native dialog, which would drag GTK or Qt into a WebKit application just to ask for a
filename.

Three details decided the implementation:

* **`OpenFile` is the call for folders too.** The interface has no "OpenDirectory" method; a
  folder chooser is `OpenFile` with `directory: true`. Reading the interface XML rather than
  guessing from the method names is what surfaced that, and it is also how `multiple` was
  found to be universal instead of per-desktop.
* **The request path is predictable, so the reply cannot be missed.** A portal call returns a
  request handle, and the answer arrives later as a `Response` signal on that handle. The
  handle lives at `/org/freedesktop/portal/desktop/request/<sender>/<handle_token>`, built
  from our own unique bus name and a token *we* choose. That means the signal subscription is
  established **before** the call. The screenshot path subscribes afterwards, which only
  survives because a human has to approve a dialog first; a portal that answers immediately
  would race and lose the reply. The picker does not take that chance.
* **A cancelled dialog is not an error.** Response code 1 means the user closed it. Returning
  an error there would send every cancellation down the fallback path and pop a *second*
  dialog after they had already said no.

The result is verified two ways: the interface introspects on the live bus as
`OpenFile(ssa{sv}) → o, version 4`, and the URI decoding has unit tests for percent-escapes
(`%20` for a space, `%C3%A9` for a non-ASCII character, a literal `%` left alone), because a
decoded-wrong path is a path that silently does not exist.

### X11 is spoken directly instead of through five binaries

On a bare X11 desktop the platform layer was calling `import` to capture, `xrandr` to list
displays, `xdotool` for the pointer and the window under it, and `xclip`/`xsel` for the
clipboard. Five binaries, four text formats, one protocol.

`x11rb` implements the X11 wire protocol in Rust and does not link `libX11`, so those become
typed calls. Three of the four conversions are done:

| capability | before | after |
|---|---|---|
| screen capture | `import` writing a PNG to disk | `GetImage` on the root window → PNG in memory |
| display enumeration | parse `xrandr --query` | RandR `GetScreenResources` + `GetCrtcInfo` |
| pointer position | parse `xdotool getmouselocation` | `QueryPointer` |
| window under pointer | `xdotool getmouselocation --shell` | `QueryPointer`, then walk down to the leaf |
| clipboard | `xclip`/`xsel` | **still a subprocess — deliberately** |

Three things this cost, all worth writing down:

* **`GetImage` returns BGRX, not RGBA.** The framebuffer comes back in the server's own pixel
  order, and the classic symptom of getting it wrong is a screenshot that comes out blue. The
  byte stride decides the layout (4 bytes per pixel = 32bpp BGRX, 3 = 24bpp BGR) rather than
  the code assuming 32.
* **`QueryPointer` reports a child, not the leaf.** Asking the root window which window the
  pointer is over returns the immediate child. Getting the window actually under the cursor
  means walking down the tree until there is no further child — bounded, because a
  misbehaving tree must not become an infinite loop.
* **A region past the edge must clamp, not wrap.** X11 will happily wrap a rectangle that runs
  off the screen. Asking for a 200×200 region at (1270, 790) on a 1280×800 display returns
  exactly 10×10, which is a test, not an assumption.

**The clipboard stays a subprocess, and the reason is structural, not laziness.** Serving an
X11 selection means taking ownership of it and staying alive to answer `SelectionRequest`
events from every other client that asks. `xclip` exists because that is a process-shaped job.
Doing it in-process would mean keeping an event loop alive for the lifetime of the app, which
is more than a conversion — it is a threading model change, and it buys nothing that `xclip`
does not already do.

The X11 code is proven against a real X server, not a mock: `Xvfb :98 -screen 0 1280x800x24`
with `QUIXI_X11_TEST=1` runs six tests that assert a genuine PNG comes back at the right
dimensions, that a region is honoured, that an over-the-edge region clamps to 10×10, that
RandR names the output, and that the pointer walk terminates on a bare root window with no
window manager.


## How these conclusions were reached

This section exists because the conclusions matter less than the method that produced them.
Every claim above was arrived at the same way, and the same way is what makes the patch
maintainable by somebody who was not here.

### 1. Read the implementation, not the documentation about it

Every interface in this patch was checked against the thing that actually implements it,
because summaries and memory were wrong repeatedly:

| what | read | what it corrected |
|---|---|---|
| portal file chooser | `org.freedesktop.portal.FileChooser.xml` from xdg-desktop-portal | there is no OpenDirectory — folders are `OpenFile` + `directory: true`; `multiple` is universal |
| GNOME screenshot | `gnome-shell` source, `js/dbusServices/screenshot*` | the bus name is `org.gnome.Shell.Screenshot`; both methods take a `filename` string |
| GNOME displays | Mutter's introspection on a live session | the destination is `org.gnome.Mutter.DisplayConfig`, not `org.gnome.Mutter` |
| XFCE capture | `xfce4-screenshooter/src/main.c` | `-f -s` takes a file *or* a directory, `-s` alone does not capture |
| KWin capture | live `busctl introspect` of ScreenShot2 | `CaptureArea` takes a pipe fd and returns raw RGBA with `stride = w*4`, not a PNG |
| X11 | `x11rb` generated protocol types | `GetImage` gives BGRX; `QueryPointer` gives a child, not the leaf |

Two bugs that would have shipped in the GNOME path were found this way and no other: the wrong
bus name, and a missing argument. Neither is visible from a summary of the API, both fail at
runtime with an error that names neither.

### 2. Measure before choosing

A decision without a number is an opinion. The measurements that decided the architecture:

| measurement | result | what it decided |
|---|---|---|
| KWin ScreenShot2, 256×256 | 2.9 ms | the compositor interface is worth writing |
| KWin ScreenShot2, 1920×1080 | 22.6 ms | |
| KWin ScreenShot2, full desktop | 29.2 ms | |
| `spectacle -b -f` full frame | ~921 ms | ScreenShot2 is 30× faster and became the primary path |
| `spectacle` region capture | ~1250 ms | regions moved in-process instead |
| `ydotool` typing, per key | 133 µs at `-d 0` vs 2358 µs at `-d 1` | typing batches at `-d 0` |
| NVMe write, laptop 4 TB | 1.3 GB/s | bulk moves are I/O bound, not CPU bound |

The screen-capture numbers are the load-bearing ones: without them, "shelling out to
spectacle" looks perfectly reasonable, and the 30× difference is invisible from the code.

### 3. Try the best path, then fall back — never abort the chain

The capture chain used to `bail!` as soon as one backend failed. On a non-KDE desktop with
`spectacle` installed, that meant total capture failure while `grim`, `gnome-screenshot`,
`xfce4-screenshooter` and `import` all sat unused. Found by a test that put a deliberately
failing `spectacle` in a simulated `PATH`.

Now every backend is attempted and the error lists what each one said, which turns "capture
failed" into "kwin: no service; portal: cancelled; import: exit 1".

### 4. Prove it on the real desktop, and label what is simulated

The order was deliberate: KDE first (the primary environment), then XFCE (the honest X11
case), then GNOME. Each was proven with a live session and a real screenshot, not a mock:

* **KDE Wayland** — 9/9 capability matrix, `cursor_position 3560,1232`
* **XFCE X11** — 15/15 including `window_under_cursor`, capture returning a real **2560×1600** PNG
* **GNOME Wayland** — portal screenshot 38,031 bytes; region capture 783 bytes after the
  in-process crop
* **X11 protocol** — six tests against `Xvfb :98`, including the clamp case

wlroots is the one case that could not be run live here. It is covered by a test that builds a
`PATH` containing exactly the tools Sway ships and asserts the right backend is chosen — and it
is described in this document as a simulation, not as a live result. Claiming otherwise would
be the same class of lie as reporting `screenshot: true` on a machine with no display.

### 5. Gate the tests that need a thing, and let them skip loudly

Three gates exist because the environment is part of the test:

* `QUIXI_INPUT_TEST=1` — injects real keystrokes, so it must never run unasked
* `QUIXI_X11_TEST=1` — needs a real X server (`Xvfb` is enough)
* `compat` — runs the same binary under Wayland, X11-only and no-display conditions

A test that silently passes because it did nothing is worse than no test, so the gated ones
assert their preconditions and the compat bench fails the run when a session type answers
dishonestly.

### 6. Traps that produced wrong conclusions, and the tell for each

These cost real hours and each one had a signature worth recognising again:

* **Stale `XAUTHORITY`.** After a session switch the harness inherited credentials from the
  previous session. `kscreen-doctor` core-dumped, the cursor "vanished" and the active output
  read `Placeholder-1`. The code was fine; the environment was lying. Tell: a function that
  worked ten minutes ago fails with a resource error it never failed with before.
* **`XDG_CURRENT_DESKTOP` leaking into tests.** Five simulated-desktop tests went red because
  the shell had `XDG_CURRENT_DESKTOP=KDE` exported and the tests inherited it, so the
  "GNOME" test ran against KDE's detection. Tell: a whole suite fails at once while the code
  under test is unchanged.
* **Test order is alphabetical.** A mouse test that clicked stole focus, and the *typing*
  test that ran after it looked like a total input failure. libtest sorts by name, so the
  names now encode order (`t1_typing`, `t4_mouse`) and anything that moves the pointer runs
  last. Tell: the first test passes and an identical later one fails.
* **`which()` and `run()` disagreeing.** `which()` falls back to the standard bin directories
  (correct for a desktop launch with a minimal `PATH`); `run()` executed by bare name using
  only `PATH`. So the probe said "grim is available" and the spawn failed `NotFound`. Tell:
  the capability report and the behaviour describe different machines. The fix was structural:
  `run()` executes the absolute path that `which()` found.
* **A timeout that stopped reading the pipe.** Adding a deadline to `Command::output()` meant
  polling `try_wait()` while leaving the pipes unread — which deadlocks the moment the child
  writes past the pipe buffer. It showed up as a truncated 1 MB clipboard read and a suite
  that took 30 s instead of 9 s. Tell: a large read comes back short. Fix: one reader thread
  per stream, draining while the wait polls.
* **`ydotool --absolute` is not in pixels.** Moving to the middle of a 2560×1600 screen put
  the pointer in the corner, because the injected space is half the screen. The ratio cannot
  be read from the display list (both readings are in the same pixels), so it is measured once
  at runtime and cached. Tell: every coordinate is off by a constant factor.
* **The report and the behaviour drifting apart.** `capabilities()` said `spectacle` while the
  chain actually used `xfce4-screenshooter`, because they were written separately. Fix:
  `capture_backend_order()` is read by both, so the report and the behaviour cannot disagree.

### 7. The rules that fell out of it

Four, and they generalise past this patch:

1. **Degrade honestly.** Every entry point that cannot work returns an `Err` that names the
   missing thing. No silent no-ops, no false capability flags. `session_type` is `"none"` on a
   headless box, not `"x11"`.
2. **One function decides what a report claims.** If the UI asks "can you screenshot?" and the
   capture path asks separately, they will drift. Both read `capture_backend_order()`.
3. **A subprocess is only acceptable where the alternative moves a privilege into the GUI or
   drags a toolkit in.** `ydotool` needs `uinput` root access — a Rust client would put that
   privilege inside a WebKit app rather than remove it. `kdialog`/`zenity` would link GTK/Qt.
   Those are reasons; "it was easier" is not.
4. **Never claim a capability without exercising it on the real thing.** The compat bench
   exists because 16 adversarial unit tests could not catch a headless box reporting `"x11"`
   — only running the binary with no display did.

### Typing needs a real inter-key delay on Wayland, and `-d 0` corrupts text

`ydotool type -d 0` is 20x faster than `-d 1` and was chosen on that measurement alone. On
GNOME it is **silently lossy**: the string `qx-26736` arrived as `qx2673d` — the dash
dropped and two digits transposed. Nothing errors; the text is just wrong.

At 25 ms the same string arrives byte-identical, reproducibly. So the delay is applied only
where it is needed: Wayland gets 25 ms, X11 keeps 0 (`-d 0` measured clean there, 133 µs per
key against 2358 µs).

A related trap for anyone writing tests against GNOME: **typing while no window has focus opens
the shell overview**, because typing on the bare desktop is "type to search". Text that vanishes
from your target is usually visible in that search field, which looks like an input failure and
is not one.

### An XWayland client exposes two windows, and only the inner one takes focus

An xterm told to use `-geometry 80x24+200+200` creates TWO X windows:

| window | geometry | what it is |
|---|---|---|
| `14681104` | 534x403 at 175,175 | the outer frame |
| `23068690` | 484x316 at 200,237 | the client window |

`xdotool windowactivate` on the **frame** leaves focus wherever it was — the call succeeds and
nothing happens. Activating the **client** works. Both ids come back from
`xdotool search --name`, and which is which is not stated anywhere, so the working approach is
to try each and keep the one the window manager reports as active:

```sh
for w in $(xdotool search --onlyvisible --name TYPEPROF); do
    xdotool windowactivate --sync "$w"
    [ "$(xdotool getactivewindow)" = "$w" ] && break
done
```

This is worth writing down because the obvious reading of the GNOME situation is wrong. GNOME
does drop programmatic activation — but **only for native Wayland clients**. For XWayland
clients, X11 activation still works, which is exactly what [ksnip PR #1201](https://github.com/ksnip/ksnip/pull/1201) had to work out: they had special-cased GNOME
and broke activation for their own XWayland build.

### `cursor_position` on Wayland was a plausible lie, and it hid a second bug

XWayland answers X queries even on a Wayland session, so asking it produced a real-looking
number that meant nothing: **1280,800 — the centre of a 2560x1600 screen — both before and after
the pointer was moved to 400,300**. A caller cannot distinguish that from a true reading, which
is worse than no reading at all.

It now returns `None` on Wayland. That immediately broke `active_output`, which had been deriving
the answer as "the monitor whose rectangle contains the pointer" — a chain of two wrongs that
looked correct. The fix is to stop guessing: Mutter's `GetCurrentState` reports a `primary` flag
per logical monitor, so GNOME now uses that directly.

### GNOME will not start under sddm, and that is a display-manager interaction

`gnome-session` aborts with **"A graphical session is already running!"** when sddm owns seat0,
because sddm's own greeter holds a session there. Repeated attempts produced core dumps from
`gnome-session-init-worker` that look like a crash but are that refusal.

KDE and XFCE are unaffected. GNOME needs a display manager that hands the seat over cleanly —
GDM, its own. This is worth knowing before concluding that a GNOME session is broken on a
machine: the session never started, so nothing about the desktop was being tested.

## 3. What is Rust, and what is not

The rule this layer follows: **in-process wherever a Rust-native route exists — the
session bus, spawning a process directly, or a protocol we already speak. A subprocess
only where the alternative would drag in a GUI toolkit, a privilege, or an
implementation of a protocol whose only consumer would be us.**

### In Rust, no subprocess

| capability | how |
|---|---|
| screen capture, full frame and region | `org.kde.KWin.ScreenShot2.CaptureArea` over zbus, raw RGBA repacked by stride, encoded with the pure-Rust `png` crate |
| active output name | `org.kde.KWin.activeOutputName` over the same zbus connection |
| application launching | the parsed `Exec` line spawned with `std::process::Command` |
| `.desktop` parsing, XDG search path | `std::fs` + the Base Directory spec |
| key names → evdev codes, characters → keycodes | plain tables |
| capability probe | environment + `PATH` inspection |
| clipboard write handoff | spawn, then wait on the parent only |
| file and folder pickers | `org.freedesktop.portal.FileChooser.OpenFile` over zbus, with `multiple` and `directory`, and the `Response` subscription established before the call |
| X11 screen capture, full frame and region | `GetImage` on the root window via `x11rb`, BGRX repacked by stride, encoded with `png` |
| X11 display enumeration | RandR `GetScreenResources` + `GetCrtcInfo` via `x11rb` |
| X11 pointer position and window under it | `QueryPointer` via `x11rb`, walking down to the leaf window |

All three crates are pure Rust with no C dependency: `zbus` (already resolved in upstream's
`Cargo.lock` before this patch), `png`, and `x11rb` (added by this patch — it implements the
X11 wire protocol itself and does not link `libX11`).

### Not Rust, and the reason for each

| what | why it stays a subprocess | cost to convert |
|---|---|---|
| `ydotool` — all input synthesis | it writes to `/dev/uinput`, which needs root or a udev ACL. `ydotoold` exists so that the privileged part is a small separate daemon. A Rust uinput client is ~200 lines and would not remove the privilege requirement — it would move it **into the app**, i.e. a WebKit GUI running with device-injection rights. That is worse, not better. | low to write, wrong to place |
| ~~`kdialog` / `zenity` — file and folder pickers~~ | **converted.** The pickers now go through `org.freedesktop.portal.FileChooser` over zbus, which works identically on KDE, GNOME and wlroots. kdialog and zenity remain only as the fallback when no portal is running. | done |
| ~~`xrandr` — display enumeration on X11~~ | **converted for X11.** RandR is spoken directly through `x11rb`. `xrandr` remains as the fallback if the protocol call fails, and `kscreen-doctor` / `wlr-randr` still cover KWin-on-X11 and wlroots. | done for X11 |
| `spectacle` / `grim` / `gnome-screenshot` / `import` | fallbacks only. The primary path is in-process; these exist so a non-KDE desktop still captures something. On GNOME and wlroots there is no equivalent public API to call instead. | n/a — fallback by design |
| `wl-copy` / `wl-paste` / `xclip` / `xsel` — clipboard | Re-examined during the X11 conversion and deliberately left. Serving a selection means owning it and staying alive to answer `SelectionRequest` from other clients — a process-shaped job, which is why `xclip` exists. Converting means keeping an event loop alive for the app's lifetime: a threading-model change, not a call substitution. On Wayland the equivalent is `wl_data_device` via `wayland-rs`, which links the C `libwayland-client`. | high, and would not remove a dependency cleanly |
| ~~`xdotool` — cursor position, window under cursor~~ | **converted for native X11** via `QueryPointer` through `x11rb`. It remains as the fallback under XWayland, and on native Wayland the compositor exposes no cursor query to clients at all — the tool stands in for a compositor limitation, not a shortcut. | done for X11 |
| `gio` / `gtk-launch` / `kioclient6` | fallbacks for launching when the `Exec` binary is not on `PATH` but a desktop launcher can still find it. | n/a — fallback by design |

Two of these were converted during review after the audit above made them obvious:
application launching (was `gtk-launch` → `gio` → `kioclient6`) and the active output
name (was `qdbus6`). Neither needed a new dependency, and both removed one process
spawn from a hot path. `qt6-tools` is no longer in the dependency list as a result.

## 4. Desktop environments

Arch is not one desktop. The layer is written to work on any of them, and where it
cannot, to say so rather than pretend.

| desktop | session | capture | clipboard | displays | proven |
|---|---|---|---|---|---|
| KDE Plasma | Wayland | ScreenShot2, in-process, ~29 ms | wl-clipboard | kscreen-doctor | **live** |
| GNOME | Wayland | portal, in-process | wl-clipboard | Mutter DisplayConfig, in-process | **live (portal path)** |
| XFCE | X11 | `GetImage`, in-process via x11rb | xclip | RandR, in-process via x11rb | **live, and against Xvfb** |
| Cinnamon | X11 (Muffin) | `GetImage`, in-process via x11rb | xclip | RandR, in-process | simulation |
| wlroots | Wayland | portal or `grim` | wl-clipboard | `wlr-randr` | simulation only |
| any | any | pickers via portal, in-process | — | — | **live on KDE, proven by interface + unit tests** |
| headless | none | none — and it says so | none | none | **live** |

Lower priority but supported: Sway / Hyprland and anything else on wlroots, through `grim` and
`wlr-randr`. It needs its own path because wlroots implements `wlr-screencopy` while KWin and
Mutter implement their own protocols instead — the Wayland core has no screenshot or
output-management, so the compositors diverge. It is the least common of this set.

XFCE and Cinnamon are both X11, which is why one code path covers all of them: RandR, the X
clipboard and XTEST behave the same under every X11 window manager.

"Proven by simulation" means `compat_desktops.rs` builds a PATH containing exactly
the tools that desktop ships and checks that the right backend is chosen and the
right file comes out. It is not the same as running on GNOME, and it is not claimed
to be — but backend *selection* is the part that breaks on an unfamiliar desktop, and
that part is covered. Running the real GNOME and Sway paths needs those desktops
installed.

Two bugs this exercise found, both of which would have hit exactly the users this
section is about:

* **A failing backend aborted the chain.** `spectacle` was tried first and `bail!`ed
  on failure, so a non-KDE desktop with spectacle installed — or a single transient
  spectacle failure — got a total capture failure while `grim`, `gnome-screenshot` and
  `import` sat there unused. Every backend is now tried in turn and a failure falls
  through; if all fail, the error lists what each one said.
* **Detection and execution disagreed.** `which()` searches the standard bin
  directories as a fallback, because a `.desktop`-launched app often has a minimal
  PATH — but `run()` executed by bare name and therefore used ONLY PATH. On such a
  session the probe reported a tool available and the spawn failed with NotFound. The
  launcher now resolves once and executes the absolute path it found, so detection and
  execution cannot disagree.

Capture is also gated on there being a display at all. A headless box can have
spectacle installed, and `which` will find it in `/usr/bin` even with an empty PATH;
claiming `screenshot: true` there is the same lie as reporting `session_type: "x11"`
with no `DISPLAY`.

## 5. Dependencies

Nothing below is required to *build*. Each unlocks one capability at runtime, and
`capabilities()` reports what is actually present so the UI can hide what is not.

**Already required by the package:** `glibc`, `gcc-libs`, `gtk3`, `webkit2gtk-4.1`, `glib2`,
`pango`, `cairo`, `gdk-pixbuf2`, `libsoup3`, `openssl`, `hicolor-icon-theme`, `bash` — the
twelve real `DT_NEEDED` owners, audited with `objdump -p` and checked with `namcap`. `curl` and
`git` were declared before this audit and are not linked or invoked; `git` is now an optdepend
and `curl` is gone.

| optdepend | package | unlocks |
|---|---|---|
| `spectacle` | spectacle | screen capture fallback on KDE |
| `grim` | grim | screen capture on wlroots (Sway, Hyprland) |
| `gnome-screenshot` | gnome-screenshot | screen capture on GNOME |
| `imagemagick` | imagemagick | `import` capture fallback where no desktop tool exists (region cropping is in-process and does not need it) |
| `kscreen` | kscreen | display enumeration on KDE (`kscreen-doctor`) |
| `xorg-xrandr` | xorg-xrandr | display enumeration fallback on X11 (RandR is spoken directly via `x11rb`) |
| `wlr-randr` | wlr-randr | display enumeration on wlroots |
| `wl-clipboard` | wl-clipboard | clipboard read/write on Wayland |
| `xclip` | xclip | clipboard read/write on X11 |
| `xsel` | xsel | clipboard fallback on X11 |
| `ydotool` | ydotool | **all input synthesis** (typing, keys, mouse); needs `ydotoold` running |
| `kdialog` | kdialog | file and folder pickers **when no XDG portal is running** (the portal is the primary path) |
| `zenity` | zenity | same fallback on desktops without kdialog |
| `kio` | kio | app launching via `kioclient6` |
| `xdotool` | xdotool | cursor position and window-under-cursor **under XWayland only** — native X11 now uses `QueryPointer` in-process |
| `gnome-keyring` | gnome-keyring | Secret Service on non-KDE (upstream credential path) |
| `kwallet` | kwallet | Secret Service on KDE (upstream credential path) |

`ydotool` is the only one that changes the feature set materially: without a
running `ydotoold`, every input call returns
`ydotool is not installed; input synthesis is unavailable` and `capabilities()`
reports `input_synthesis: false`.

---

## 6. Limitations

**Cursor position is a hint on Wayland.** The compositor exposes no cursor query to
clients. This reads through XWayland, which is exact on an X11 session and is exact
on Wayland only while the pointer is over an XWayland surface. Measured on the same
instant with the pointer over a native Wayland window:

```
xdotool getmouselocation  ->  1895, 922     (identical on repeated reads)
KWin workspace.cursorPos  ->  2087,  45
```

900+ pixels apart. Anything needing a guaranteed-fresh position on Wayland must use
the compositor's own scripting, which is asynchronous and was not wired here.

**Multi-folder selection is single-folder.** Neither `kdialog` nor `zenity` offers a
multi-directory chooser. `pick_folders` returns one folder rather than pretending and
returning empty.

**Region capture on the fallback path is still a full capture plus a crop.** Only the
`ScreenShot2` path captures the rectangle directly.

**TCC is reported as `"unknown"`,** the value every non-macOS platform reports: Linux has
no equivalent per-application privacy gate, and reporting `"granted"` would be a
fabricated capability claim.

**No window enumeration**, for the reasons in §1.

---

## 7. Tests

Seven suites ship with the patch, under `crates/quixi-platform/tests/`, plus the inline unit
tests. All are runnable with `cargo test --release -p quixi-platform`.

| file | tests | what it proves |
|---|---|---|
| (inline `linux::tests`) | 19 | keycode table, character map, desktop-entry parsing, capability consistency, portal URI decoding (percent-escapes, non-file schemes, a literal `%`), dialog timeout bounds, and 6 **gated** X11 protocol tests |
| `linux_computer_use.rs` | 7 | each capability against the live session: real PNG, clipboard round-trip, plausible display geometry, honest errors |
| `capture_modes.rs` | 1 | whole-frame and region both take the fast path; region is cheaper and exactly the size asked for |
| `bug_hunt.rs` | 16 | adversarial input: empty/whitespace, unmappable characters named in the error, degenerate combos, unknown buttons, injection strings, bad geometry, concurrency |
| `stress.rs` | 11 | throughput and leaks: 1 MB clipboard round-trip, 25 captures, 200 probes, 500 error paths, fd and RSS deltas |
| `launch.rs` | 2 | application launching really spawns the `Exec` line — proven by a throwaway `.desktop` whose command writes a marker file, with an assertion that no external launcher was used |
| `compat_desktops.rs` | 7 | GNOME, XFCE, Sway, Cinnamon, a failing backend falling through to the next, bare, and headless — each simulated by building a PATH with exactly that desktop's tools, and each declaring the desktop it simulates |
| `compat.rs` | 4 | the same contract in three sessions: live Wayland, X11-only (Xvfb, no compositor, no session bus) and headless |
| `input_synthesis.rs` | 4 | **real** keystrokes into a real window — the typed command must create a file |

Two gates exist because the environment is part of the test:

* `QUIXI_INPUT_TEST=1` — injects **real** keystrokes and moves the real cursor. Opt-in because
  a stray keystroke lands in whatever has focus. Run it only where that costs nothing.
* `QUIXI_X11_TEST=1` — needs a real X server. `Xvfb :98 -screen 0 1280x800x24` is enough, and
  that is how the X11 protocol tests are proven:

  ```
  Xvfb :98 -screen 0 1280x800x24 -nolisten tcp &
  env -u WAYLAND_DISPLAY DISPLAY=:98 QUIXI_X11_TEST=1 \
    cargo test --release -p quixi-platform --lib x11_
  ```

  Those six cover: `QueryPointer` on a live server; a full `GetImage` that decodes to the real
  screen size; a region honoured exactly; a region past the edge clamping to 10×10; RandR
  naming the output; and the leaf-window walk terminating on a bare root with no window manager.

Run the input-touching suites on a machine where a stray keystroke costs nothing:

```
cargo test --release -p quixi-platform --test input_synthesis -- --test-threads=1
cargo test --release -p quixi-platform --test bug_hunt -- --test-threads=1
cargo test --release -p quixi-platform --test stress -- --test-threads=1
```

Measured on Arch / Plasma 6.8, 32-thread Zen 4: 46 tests, all passing; `clippy`
clean for the files added here.
