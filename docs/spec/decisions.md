# Project decisions

Decisions that override or narrow the specs in this folder. When a spec requirement conflicts with
an entry here, this file wins.

## Decided by the project owner

| # | Decision | Effect on the specs |
|---|---|---|
| D-1 | Reimplement the original's form layouts and logic faithfully; the original's resources (bitmaps, icons, translations, map assets) may be reused. | Specs are the source of truth for layout and behaviour; assets come from the original exe. |
| D-2 | No compatibility with existing installations: do not read or import the original's registry settings, file-list cache or other per-user state. | Settings use the new app's own storage; requirements about migrating or reading `HKCU\Software\DATAKAM PLAYER` are out of scope. Default values from the settings table still apply. |
| D-3 | Languages: English and Russian only. | Only `TRANSLATE_EN` and `TRANSLATE_RU` (and their brand/fix overlays if needed) are shipped; the Language menu lists these two. |
| D-4 | Pure Rust; no external tools at runtime. Video decoding uses the vendored FilmCraft decoders. | DirectShow-specific requirements (splitter/decoder/renderer selection) are replaced, not reproduced. |

## Defaults chosen for open questions (change if the owner disagrees)

| # | Question | Default |
|---|---|---|
| Q-1 | Telemetry, ads panel, analytics, self-update, firmware download | Dropped (servers are gone; not needed). |
| Q-2 | Single-instance behaviour | Keep the original: none. |
| Q-3 | DirectShow Splitters / Decoders / Renderer menus | Removed; replaced later by a hardware/software decoding choice if needed. |
| Q-4 | Bugs found in the original | Fixed, and noted next to the requirement as "original bug, fixed". |
| Q-5 | Embedded third-party repair tool ("Full" repair) | Not reused (unknown origin and licence); repair uses the app's own header-rebuild logic. |
| Q-6 | Position divided by play rate under DirectShow (playback-video Q1) | Not copied: the position always moves at the real play rate. |
| Q-7 | Auto-next only at exactly p = 1.0, wrapping from last row to first (playback-video Q2) | Triggered by end of stream; wrap-around kept as in the original. |
| Q-8 | "Back Frames" is a 100 ms seek back (playback-video Q3) | True one-frame step back (the new decoder can do it); label unchanged. |
| Q-9 | Left/Right jump even when the file list has focus (playback-video Q8) | Kept as in the original. |
| Q-10 | Map providers (most embedded tile services/APIs no longer work) | Keep only working, policy-compatible OpenStreetMap-based maps; Google/Yandex/Navitel/Baidu menu items removed. |
| Q-11 | Switching to a GPS profile selects Google Maps | Selects the default OSM map instead (Google is no longer available). |
| Q-12 | Map A/B markers mark the timeline zoom-window edges, not the loop selection | Kept as in the original (it is deliberate); the hide-B-at-finish comparison bug is fixed. |
| Q-13 | Empty strip / empty right column when the control panel or both list and map are hidden | Treated as original bugs and fixed: the space is given to the remaining panels. |
| Q-14 | Gauges keep the last reading when GPS data disappears | Fixed: gauges show no reading when there is no data at the position. |
| Q-15 | Speed dial is 0–200, unclamped, same dial for mph | Dial bitmap kept; the needle is clamped to the dial range; mph values use the same dial scale. |
| Q-16 | Timeline zoom scrollbar off by one | Fixed. |
| Q-17 | Remove / Clear / Ungroup / Ungroup All do not refresh the row count until the next background notification | Fixed: the list refreshes immediately. |
| Q-18 | After a header-click sort the selection stays on the same row number | Fixed: the selection follows the same item. |
| Q-19 | Next/previous segment lookup bumps the last number in the full path including the extension (one sidecar rule never fires for `.MP4`) | Fixed: the segment number is taken from the file name without extension. |
| Q-20 | Settings persistence (column widths/order, sort, overlap time, panel sizes) | Persisted in the new app's own settings file, using the original's defaults (see D-2). |
