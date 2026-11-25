# Mechanics sheet — what the original does, and where it was read

The installer the person gave (`SideloadlySetup64 (1).exe`) is an NSIS
package. Its real program was read from the copy that installer put on this
PC: `%LOCALAPPDATA%\Sideloadly\sideloadly.exe` (Go 1.23 + Qt, 64-bit,
sha256 28e95c45…). Function names come from Go's own function table; the
code was read with Ghidra (Deep mode) and objdump. Addresses are virtual
addresses in that file. Nothing was copied: this sheet describes the rules,
and the app's code is written from it.

The actual signing and installing runs in the original's packed Python part
(`sideloadly.dll` is UPX-packed). It was **not** read. That part of this
app comes from the open-source isideload library (vendor/isideload). The
person confirmed it works.

## Window size — `gui.adjustWindowSize` @ 0x179fb40

| value | number | where |
|---|---|---|
| screen DPI if none reported | 96.0 | constant 0x390dc08 |
| width factor | 7.8125 px per DPI | constant 0x390dc00 |
| minimum layout width | 750 px (= 96 × 7.8125) | `cmp $0x2ee` at 0x179fc32 |
| final width | layout width × 8 / 10 → **600** at 96 DPI | `imul 0xcccc…cd`, `sar 3` at 0x179fc8d |
| height | the layout's natural height, then the window is fixed to that size | `AdjustSize`, `SetFixedSize` |

Re-run every time an option changes (`updateStartEnabled` calls it last).

In this app the width starts at 600 points. egui points already scale with DPI,
so this is the same as 7.8125 × DPI × 0.8. While the status history is hidden,
the height fits the content and refits when the content changes. The window
stays resizable and full-screen capable (F11) because the person asked for
that earlier.

## Start button — `gui.updateStartEnabled` @ 0x17a0320, `gui.makeStartBtn` @ 0x179e2c0

* The button text is "&Start", so **Alt+S** presses it.
* It is enabled only when:
  * nothing is running (flag 0x5768fd3), and
  * an app file is chosen (0x571fe98 / 0x571f100), and
  * a device is chosen, unless the mode doesn't need one (mode 1 = export) (0x571f0f8, 0x5769a08), and
  * every ticked option that needs text has text (four flag/field pairs at 0x5768fc8+4..7).
* The original's Cancel button (c / Alt+C) and Enqueue button (q) are **not in this simpler
  version**. The person asked for one app at a time, simpler.

## Progress — `gui/progress.(*Tracker).Update` @ 0x16fe2c0, `.view` @ 0x16fe880, `SplitMessage` @ 0x16fe080

A progress message names a *step* (channel) and optionally a detail:

* **Split** (`SplitMessage`): look for `": "` in message+" " and for `"…"`. Whichever
  comes first cuts it. Step = trimmed text before the cut, detail = trimmed text after
  it (without the `:` or `…`). With neither, the whole trimmed message is the step.
* Each step remembers done, total and a **ratio**: ratio = max(old ratio, min(done/total, 1.0)).
  It never goes back, and 1.0 is the cap (constant 0x390dd18).
* **Weight** of a step = its total (or its done when total is 0).
* **Overall** = Σ(ratio × weight) / Σ weight (0 when Σ weight ≤ 0). It never goes back
  (`haveUnified`/`lastUnified`).
* **Bar**: maximum 10000, value = int(0.5 + 10000 × overall) (constants 0x390dd00 = 0.5,
  0x390dd58 = 10000).
* A step is *active* while total = 0 or done < total. The status label lists the active
  steps that have started (done ≠ 0), joined with `" & "`.
* Step percent text = int(100 × ratio + 0.5) + "%" (0x390dd50 = 100, 0x390dd00 = 0.5).
* **Log line** per step: `"<step>: <pct>"` plus `" <detail>"` when there is a detail.
  It is written only when done ≠ 0 and (the step is new, the detail changed or the
  percent changed). When the detail is unchanged it **replaces that step's previous
  line in place** instead of adding a new one (`logDetail` @ 0x17a25c0 with
  `lastLineIndex` @ 0x17a2c40: the last *whole line* equal to the old text is swapped).

`gui.setUnifiedProgress` @ 0x17a3000: label = steps + `"… "` + detail (bytes
`e2 80 a6 20` at 0x377cbd5). If only one of them exists, that one is the label. Percent
text = value × 100 / max with integer division, `"%d%%"`. It is empty when max = 0.
The same rule is in `gui.setProgress` @ 0x17a2d80.

The step names and sizes the original's Python part sends are **not known**
(see unknown unk_85d5f7f0cac625f9). This app's steps and sizes are its own:

| mode | steps (total = weight), all **estimated** |
|---|---|
| install | Preparing app 5 (only when options change the app) · Signing in 10 · Developer account 5 · Signing 60 · Installing 100 |
| export | Preparing app 5 · Signing in 10 · Developer account 5 · Signing 60 · Saving 20 |
| check Apple ID | Signing in 10 · Developer account 5 · Certificates 5 · App IDs 5 |

## Status row and log — `gui.makeStatusBox` @ 0x17a13c0, `gui.logLine` @ 0x17a2380

* The history box starts with the text **"Idle."** and is **hidden** at start
  (`SetVisible(false)` at 0x17a1441). A toggle button with the tooltip "Show status
  history (Alt+.)" and the shortcut **Alt+.** shows it.
* The status label shows the newest log line, cut to **128** bytes + "…" when longer
  (`logLine.func1` @ 0x17a2520).
* A small label beside it shows the percent text. The progress bar follows it.
* Font families for the history: Helvetica, Arial, sans-serif (0x377f537, 0x377d147,
  0x377fe33). This app uses egui's own monospace font, so that list is not used.
* The log only follows new lines when the view is less than one line-height from the
  bottom (`keepStatusboxBottom` @ 0x17a29e0). egui's "stick to bottom" behaves the same.
* Each log line also goes to the program's output file. Secrets are hidden first when
  "hide sensitive info" is on (`helpers.CleanupText` @ 0x16a1620). This app never logs
  passwords and masks the Apple ID in its own lines.

## Saved options — `gui/helpers.(*Option).Load` @ 0x16a0dc0

* Every option is its own file, `option-<name>`, in the user's data folder.
* At most **512** bytes are read. A missing file means "not set".
* This app keeps its options the same way in `%APPDATA%\ipabazooka\option-<name>`. It
  remembers whether the status history is shown in `option-show-history`.

## Layout — `gui.makeForm` @ 0x17ad260, `gui.makeFileBox` @ 0x1794900

* Left: a large app-icon button with the file name under it. The name starts as
  "<none>", greyed out, one line tall. Files can be dropped on the whole window.
* Right: "iDevice:" with the device list, then "Apple ID:" with the account box,
  then "Advanced Options" (Alt+V), which is collapsed at start.
* Under those: the Start row. Under that: the status row and the history.
* Wi-Fi devices have a UDID that ends in "-w" (`makeDeviceBox` @ 0x17934c0). The
  original listens for plug/unplug events. This app checks every 4 seconds instead,
  which is equivalent for the person.

## Advanced options — `gui.makeAdvancedBox` @ 0x17a51c0

The original has: older iOS (7+), remove supported-device limits, file sharing, remove
app extensions, change name, change version, bundle ID (automatic by default),
custom entitlements, tweak injection and Info.plist fields.
This simpler version keeps: name, bundle ID (automatic when empty), remove device
limits, file sharing and remove extensions, plus its own sign-in options.

## "New here?" guide (this app's own addition, not in the original)

The person asked for a beginner's guide, off to the side and out of the main area.
A small "❓ Help" button in the top bar, next to the theme picker, opens a drop-down
that floats over the page. It has four parts that open on click: What is
sideloading?, Getting an app file (.ipa) including tweaked apps, Using this app
step by step, and If something goes wrong.

* The button is highlighted until the guide has been opened once (`option-help-seen`).
* While the guide is open, a short window grows to at least 620 points tall so the
  guide has room. When it closes, the window goes back to its size.

## Auto-refresh (not in this version) — `gui.makeStartBtn` @ 0x179e2c0

The original can re-install an app before its 7 days run out. The first try comes
**4 days** before expiry (0x390dbf8), and the custom timeout defaults to **96 hours**
(0x057690d0 = 0x60). This version has no background service, so this is left out.
