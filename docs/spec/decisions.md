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
| D-5 | (2026-10-29) Use existing C/C++ code or libraries where that is possible and justified, instead of reimplementing everything in Rust. | Still no external tools at runtime; a C/C++ dependency needs a compatible licence and a reason (maturity, size of the job). UI-side logic may stay in the Qt/C++ layer. |
| D-6 | (2026-10-29) The map is a native MapLibre map with vector maps, not an embedded web page; the package stays small (no browser engine). | The Leaflet page, its JS API and callback protocol (GM-151…GM-153) become a widget API with the same behaviour; see Q-27. MapLibre Native for Qt (BSD-2) is built from source by `qt/maplibre.cmd` and linked statically. |

## Defaults chosen for open questions (change if the owner disagrees)

| # | Question | Default |
|---|---|---|
| Q-1 | Telemetry, ads panel, analytics, self-update, firmware download | Dropped (servers are gone; not needed). |
| Q-2 | Single-instance behaviour | Keep the original: none. |
| Q-3 | DirectShow Splitters / Decoders / Renderer menus | Removed; replaced later by a hardware/software decoding choice if needed. |
| Q-4 | Bugs found in the original | Fixed, and noted next to the requirement as "original bug, fixed". |
| Q-21 | G-sensor readings stored twice (some cameras, e.g. the "AA" files: 5 Hz data at 10 Hz) make every other G value 0 | Fixed: a reading identical to the previous one within 0.25 s is dropped before G is computed. |
| Q-5 | Embedded third-party repair tool ("Full" repair) | Not reused (unknown origin and licence); repair uses the app's own header-rebuild logic. |
| Q-6 | Position divided by play rate under DirectShow (playback-video Q1) | Not copied: the position always moves at the real play rate. |
| Q-7 | Auto-next only at exactly p = 1.0, wrapping from last row to first (playback-video Q2) | Triggered by end of stream; wrap-around kept as in the original. |
| Q-8 | "Back Frames" is a 100 ms seek back (playback-video Q3) | True one-frame step back (the new decoder can do it); label unchanged. |
| Q-9 | Left/Right jump even when the file list has focus (playback-video Q8) | Kept as in the original. |
| Q-10 | Map providers (most embedded tile services/APIs no longer work) | Owner (2026-10-29): more providers, including a satellite view. The map offers keyless raster layers usable with attribution (and vector maps, Q-27): OpenStreetMap (standard, Humanitarian), Esri satellite, hybrid, streets and topographic, OpenTopoMap, CyclOSM. CARTO was dropped: outside a browser page its tiles now only say "API key required" (OpenFreeMap Positron is the light map). Google/Yandex/Navitel/Baidu need API keys or forbid direct tile access, so their menu items stay removed. |
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
| Q-22 | Fast repair (SF-REPAIR-017) wrote into the broken file before and during the scan; a cancel or an error could leave a half-written header | Fixed: nothing is written until the end, a cancel or error leaves the file untouched, a failed write is backed out. Also: `co64` above 2 GiB, H.265 and H.264 without delimiters. The Full method stays hidden (Q-5). |
| Q-23 | Export at a speed other than 1 kept B-frame composition offsets unscaled (frame order scrambled) | Fixed: `ctts` is scaled with the speed. The meta track follows the frames actually written (key-frame start), so GPS stays in sync. A cancelled export removes its partial output; writing over a source is refused. |
| Q-24 | KML `begin`/`end`/`when` were local time marked "Z" | Fixed: written in UTC. Other writer quirks (DatDOD +1 g on z, truncated altitude/speed, GPX element order) are kept for compatibility with programs that read these files. |
| Q-25 | Regional formats of exported text (CSV decimal separator, SRT date/time) | Taken from the system locale, as the original took them from the Windows regional settings. |
| Q-26 | SRT subtitles of an export were built from each item's source track | Built from the track written into the output video, so the cues follow its frames exactly. |
| Q-27 | The map without a web page (D-6) | Base layers: OpenFreeMap vector styles (Liberty, the default, Bright, Positron, Dark, Fiord; no key) and the Q-10 raster layers, chosen from the map's own layer button and kept in `gm_L`. The page's controls are native: zoom, layers, the distance tool (same behaviour and icons), scale, attribution. North-up only (no rotation or tilt), as the original. Zoom values (`gm_Z`, `{z}`) keep the Leaflet scale (MapLibre's zoom + 1). A route click seeks when the nearest track point is within 10 pixels (the page also required the click to be on the line). KML/GPX overlays show their lines and points without the files' own styles. "Run script after map load" (GM-171) is hidden: there is no page to run it in. |
