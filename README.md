# RV Player

A dashcam video player for Windows: plays recordings next to their GPS track on a map, with
speed and accelerometer graphs, and saves fragments, repairs broken files and converts GPS logs.

RV Player is a new program based on **Registrator Viewer** by **Vadim Andreevich Kozlov**
(Saint Petersburg), a Delphi player that is no longer maintained. It keeps the original's
windows, menus and behaviour, and rebuilds them on Qt 6 and a core written in Rust.

## Download

Get `RVPlayer-<version>-win64.exe` from [Releases](https://github.com/mrkhrd/rvplayer/releases/latest).
It is a single portable file of about 10 MB for Windows 10 and 11 (x64). There is nothing to
install, and no codecs are needed.

## Features

- **Video:** plays H.264 and H.265 (HEVC) recordings with a built-in decoder. Seeking is
  smooth, and the files of one trip play as one recording.
- **GPS:** reads tracks embedded in the videos and from the cameras' log files.
  - Embedded tracks: VIOFO, Novatek-based cameras, 70mai (with the G-sensor and altitude from
    the card's `GPSData*.txt` logs), and others.
  - Log files: about thirty formats from the original, including NMEA, GPX, KML, PLT (OziExplorer),
    DOD, Samoon, CamBox, RoadHawk and Lukas.
- **Map:** a native vector map (MapLibre), sharp at any zoom and on high-resolution screens.
  - Layers: OpenFreeMap styles, OpenStreetMap, satellite and hybrid (Esri), topographic and
    cycling maps.
  - CARTO maps work with your own free API key.
  - Click the route to jump the video to that point. There are also a ruler, a scale bar,
    KML and GPX overlays, and following the car.
- **Graphs:** speed, altitude and the accelerometer, kept in step with the video.
- **Fragments:** cuts, joins and speeds up recordings without re-encoding. The GPS track and the
  recording time are kept, and SRT subtitles are made from your own template.
- **Repair:** fixes recordings cut off by a power loss, using a good file from the same camera.
- **GPS converter:** KML, PLT, GPX, DatDOD, CSV and SRT. You can also export a track, open it
  in Google Earth, attach a track to videos and shift videos in time.
- **Other tools:**
  - file properties;
  - copying with the dates kept;
  - snapshots with the GPS position in EXIF;
  - copying the coordinates with a template.
- **Fast reopening:** what was read from each video is remembered, so a folder opens at once the
  next time, even from a memory card.
- **Languages:** English and Russian.

The original's Google, Yandex, Navitel and Baidu maps, its DirectShow codec menus, camera setup
and online updates were left out. These maps are closed to other programs or need keys, and the
original's servers are gone.

## Building

You need:

- Windows 10 or 11 (x64);
- Visual Studio 2022 (or Build Tools) with the C++ workload, which also provides CMake and Ninja;
- Rust: [rustup](https://rustup.rs) installs the version pinned in `rust-toolchain.toml`;
- Qt 6.10.3 for MSVC 2022 64-bit, in `C:\Qt\6.10.3\msvc2022_64`. For another location, set
  `QTDIR`;
- Git.

The scripts in `qt\` do the rest:

| Command | What it does |
|---|---|
| `qt\maplibre.cmd` | Fetches and builds MapLibre Native for Qt into `deps\maplibre`. Run it once. |
| `qt\build.cmd` | Builds the program and the UI tests in `qt\build` (the Rust core is built by Cargo from CMake). |
| `qt\run.cmd` | Runs the built program with the Qt DLLs on `PATH`. |
| `qt\deploy.cmd` | Makes a portable folder, `dist\RVPlayer`, and `dist\RVPlayer-win64.zip`. |
| `qt\single.cmd` | Makes the single file, `dist\RVPlayer.exe`, packed with UPX. |

`qt\single.cmd` needs no installed Qt. The first time, it downloads and builds a static Qt
(the base module) and MapLibre into `deps\`, which takes an hour or more. Later runs reuse them.
All downloads are pinned and checked against their checksums.

### Tests

```bat
cargo test --workspace --exclude rv-app
```

The UI tests (`qt\tests\tst_mainform.cpp`) drive the real windows. They need a folder of dashcam
recordings:

```bat
set RV_TEST_DIR=D:\path\to\recordings
set PATH=C:\Qt\6.10.3\msvc2022_64\bin;%PATH%
qt\build\rvplayer_tests.exe
```

Set `RV_TEST_SHOTS=<folder>` to save the windows as PNG files at checkpoints.

### Continuous integration

`.github/workflows/windows.yml` runs on every push to `master`. It builds the single file with
`qt\single.cmd`, runs the Rust tests and publishes the program as a release. The static Qt and
MapLibre are cached between runs.

## Repository layout

| Path | Contents |
|---|---|
| `crates/rv-core` | Video containers, GPS formats and writers, tracks, the file-info cache |
| `crates/rv-video` | Decoding and playback |
| `crates/rv-fm` | The file list: scanning folders and grouping trips |
| `crates/rv-edit` | Saving fragments without re-encoding |
| `crates/rv-repair` | Repairing cut-off recordings |
| `crates/rv-ffi` | The C interface the Qt program uses (`include/rv_ffi.h`) |
| `crates/rv-app` | An earlier egui interface, no longer used |
| `qt/` | The Qt Widgets program; its windows are built from the original's form files (`qt/src/vcl`) |
| `vendor/filmcraft` | The H.264 and H.265 decoders |
| `assets/` | The original's forms, images and translations, and the new icon |
| `docs/spec` | Specifications; the design decisions are in `docs/spec/decisions.md` |

## Credits

- **Registrator Viewer**, by Vadim Andreevich Kozlov. RV Player follows its design. Its window
  layouts, images and translations are reused from version 6.0.0.22.
- **[FilmCraft](https://github.com/storytold/filmcraft)** H.264 and H.265 decoders
  (MIT OR Apache-2.0). They are vendored with their licences in `vendor/filmcraft`.
- **[MapLibre Native for Qt](https://github.com/maplibre/maplibre-native-qt)** (BSD 2-Clause).
- **[Qt](https://www.qt.io)** (LGPL v3). The single-file build links Qt statically. This
  repository holds everything needed to rebuild it against a modified Qt.
- **[UPX](https://upx.github.io)** packs the single file.

### Map data

- © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors.
- Vector tiles by [OpenFreeMap](https://openfreemap.org), schema by [OpenMapTiles](https://openmaptiles.org).
- Imagery and maps: © Esri and its data providers, [OpenTopoMap](https://opentopomap.org)
  (CC-BY-SA), [CyclOSM](https://www.cyclosm.org), [Humanitarian OSM Team](https://www.hotosm.org),
  [CARTO](https://carto.com).

Each layer shows its attribution on the map. Use of each tile service is subject to its
provider's terms.

## License

Not chosen yet. The Rust crates declare `MIT OR Apache-2.0` in `Cargo.toml`. The resources taken
from Registrator Viewer remain its author's.
