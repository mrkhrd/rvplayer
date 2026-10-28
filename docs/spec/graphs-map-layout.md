# Registrator Viewer 6.0.0.22: Graphs, Gauges, Map and Main-Window Layout

Behavioural specification, reconstructed from the recovered Delphi 7 binary: decompiled units, DFM resources, debug symbols and disassembly of `RegistratorViewer_unpacked.exe`.

Citations use the form `Unit: Class.Method Lnnn–Lmmm`, where the numbers are **original Pascal source lines** (the `/* Lnnn */` markers in the decompiled code). Constants written as `x 1.1` are 80-bit extended floats read from the binary.

---

## 1. Overview

The main window (`TFormMain`) has two columns separated by a vertical splitter.

- **Left column** (`PanelLeft`): the video (`PanelVideo`) on top. Below it, a horizontal splitter and the bottom panel (`PanelBottom`), which holds:
  - the timeline zoom bar;
  - the seek bar (`ImageSeek`);
  - zero or more stacked data graphs (speed, altitude, G-sensor series);
  - in the GPS profiles, the control panel with the three gauges (`ImageSpeed`, `ImageGSensor`, `ImageAzimut`).
- **Right column** (`PanelRight`): an embedded web-browser map on top, a splitter, and the file list. In the video-only profile the control panel sits at the bottom of this column instead.

A 100 ms (configurable) timer drives everything that follows playback: the seek-bar pin, the graph cursors, the gauges, the map's current-position arrow and map auto-follow.

All values shown come from `TMovingInfo`. It cleans and interpolates the GPS and G-sensor streams, derives speed, heading and track distance where they are missing, and bins the samples per pixel column for the graphs.

The map is an HTML/JS page: Leaflet in the default provider "OSM2". The app drives it with JS calls, and the page reports back through fake `http://callback/...` navigations. The Rust app can replace this with a native map widget, but it must keep the same state machine: viewport, zoom, follow logic, route simplification and markers.

Three "profiles" switch the whole layout between *GPS + G-sensor*, *GPS only* and *video only*.

---

## 2. UI elements

### 2.1 Component tree with DFM geometry

DFM design size is 800×600 (client 784×542). At runtime the window is resized (GM-010).

The form has `KeyPreview = True` and `Constraints.MinWidth = 350`, `Constraints.MinHeight = 300`. The font is MS Sans Serif, Height −11 (8 pt), and the background is `colorMainBG` (`clBtnFace`).

Align codes used below: alNone = 0, alTop = 1, alBottom = 2, alLeft = 3, alRight = 4, alClient = 5.

| Component | Parent | DFM Align | DFM L,T,W,H | Notes |
|---|---|---|---|---|
| `SplitterRight` (TSplitter) | form | alRight | 267,0,7,542 | MinSize 10, AutoSnap off, ResizeStyle rsUpdate. Width forced to 0 or 7 at runtime (GM-017). |
| `PanelRight` | form | alRight | 274,0,510,542 | bvNone. Width persisted as `panel_Right`. |
| ─ `PanelControl` | PanelRight | alBottom | 0,410,510,132 | Re-parented by the profile (GM-020). |
| ── `ImageSpeed` (TImage) | PanelControl | none | 118,29,134,100 | AutoSize, ShowHint. Speed gauge. |
| ── `ImageGSensor` (TImage) | PanelControl | none | 260,6,65,65 | ShowHint. G-sensor gauge. |
| ── `ImageAzimut` (TImage) | PanelControl | none | 264,72,57,57 | ShowHint. Compass gauge. |
| ── `PanelControl_s1` | PanelControl | none | 4,4,105,48 | Play (48×48) and Stop (48×48 at x=57) buttons. Out of scope except for layout. |
| ── `PanelControl_s2` | PanelControl | none | 4,64,105,23 | Frame prev/next and video zoom buttons, 23×23 each. |
| ── `PanelPlaySpeed` | PanelControl | none | 4,90,108,17 | Play-speed buttons (out of scope). |
| ── `TrackBarVolume` | PanelControl | none | 0,108,113,24 | Range 0..100, ThumbLength 15. |
| ── `CheckBoxLoop` | PanelControl | none | 118,6,105,17 | Caption "Loop A-B". |
| ─ `PanelRight2` | PanelRight | alClient | 0,0,510,410 | |
| ── `SplitterListView` | PanelRight2 | alBottom (runtime alTop) | 0,169,510,7 | MinSize 1. |
| ── `WebBrowser` (TWebBrowser) | PanelRight2 | alBottom (runtime alTop) | 0,176,510,234 | The map. |
| ── `WebBrowserAds`, `WebBrowserAnalytics` | PanelRight2 | alTop | height 7 | Forced to height 0 and invisible (GM-011). Do not reimplement. |
| ── `PanelListView` | PanelRight2 | alClient | 0,14,510,155 | |
| ─── `PanelFileListMenu` | PanelListView | alTop | h 20 | File-list toolbar (other spec). |
| ─── `ListView` | PanelListView | alClient | | File list (other spec). Columns 3 = "km" and 4 = "km/h" are affected by the profile. |
| `PanelLeft` | form | alClient | 0,0,267,542 | |
| ─ `SplitterBottom` | PanelLeft | alBottom | 0,350,267,7 | MinSize 42. Visible only when at least one graph is on. |
| ─ `PanelBottom` | PanelLeft | alBottom | 0,357,267,185 | Height persisted as `panel_Bottom`. Mouse events go to the seek handlers. |
| ── `PanelGraphicsAndZoom` | PanelBottom | alClient | 0,0,198,185 | |
| ─── `PanelZoomScroll` | PanelGraphicsAndZoom | alTop | h 17 | Holds `ButtonZoomOut` "-" (17×17, flat, left), `ScrollBarZoom` (Ctl3D off) and `ButtonZoomIn` "+" (17×17, flat, right). |
| ─── `PanelBottomGraphics` | PanelGraphicsAndZoom | alClient | 0,17,198,168 | PopupMenu = `PopupMenuGraphics`. Mouse goes to the seek handlers. |
| ──── `ImageSeek` | PanelBottomGraphics | alTop | h 40 | Replaced at start-up by a TImage subclass with mouse-leave support (GM-060). |
| ──── graph images | PanelBottomGraphics | none (laid out manually) | | One per enabled graph (GM-031). |
| ── `PanelGraphicsAddon` | PanelBottom | alRight | 198,0,69,185 | Empty host. In the GPS profiles it receives `PanelControl` (GM-020). |
| ─ `PanelVideo` | PanelLeft | alClient | | Black at runtime (other spec). |
| `Timer` | | | Interval 100, initially disabled | Main UI refresh timer (GM-090). |
| `PopupMenuGraphics` | | | | Context menu for the seek bar and graphs (GM-005). |
| `OpenDialogKML` | | | | KML/GPX overlay picker (GM-172). |

Runtime override: `N_View_FileListClick` re-aligns the map to **alTop** above the file list, with `SplitterListView` directly under it (GM-016). The DFM alBottom values are never used.

### 2.2 Menu items in this area

Captions are TRANSLATE_EN.TXT keys, with the English text in quotes.

The DFM column gives the design-time state, which is also the "default" used for persistence (GM-002). Shortcut codes are Delphi TShortCut values: low byte = VK code, +$2000 Shift, +$4000 Ctrl, +$8000 Alt.

**View ▸ Profile** (`N_View_Profile`, "Profile")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Profile_GPS_G | "My dash cam has GPS and G-Sensor" | radio, **checked** | — |
| N_View_Profile_GPS | "My dash cam has GPS" | radio | — |
| N_View_Profile_Video | "My dash cam can record only video" | radio | — |

**View** (top level)

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_FileList | "File List" | checked | 113 = F2 |
| N_View_ControlPanel | "Control Panel" | checked | 115 = F4 |
| N_View_Details | "Details" | checked | 119 = F8 |

**View ▸ Maps** (`N_View_Maps`, "Maps")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_OpenStreetMaps2 | Runtime caption "[Leaflet]:  Open Street Maps + OVI + Bing + Yahoo + ..." (DoTranslate L4669) | radio, **checked** | — |
| N_View_OpenStreetMaps | Runtime caption "[OpenLayers]:  Open Street Maps + OVI + Bing + ..." (L4668) | radio | — |
| N_View_GoogleMaps | "Google Maps" | radio | 114 = F3 |
| N_View_YandexMaps | "Yandex Maps (detailed for Russia)" | radio | — |
| N_View_YandexMaps2 | "Yandex Maps 2.0" | radio | — |
| N_View_NavitelMaps | "Navitel Maps (detailed for Russia)" | radio | — |
| N_View_BaiduMaps | "Baidu Maps (detailed for China)" | radio | — |
| N_View_NoMaps | "None" | radio | — |
| (separator) | | | |
| N_View_Maps_PostAction | "JavaScript: Do After Map Loaded" | — | — |
| N_View_Maps_OpenKML | "Overlay vector image (KML)" | — | — |

**View ▸ Arrow on Map** (`N_View_Arrow`, "Arrow on Map")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Arrow_Small | "Small" | radio | — |
| N_View_Arrow_Normal | "Normal" | radio | — |
| N_View_Arrow_Big | "Big" | radio, **checked** | — |

**View ▸ Time Points** (`N_View_TimePoints`, "Time Points")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_TimePoints_1m | "1 minute" | radio | — |
| N_View_TimePoints_5m | "5 minutes" | radio | — |
| N_View_TimePoints_20m | "20 minutes" | radio | — |
| N_View_TimePoints_Never | "Don't show" | radio, **checked** | — |

**View ▸ Fix Coordinates** (`N_View_Coordinates`, "Fix Coordinates")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Coord_FlipX | "Flip/Reflect X (Longitude)" | check | — |
| N_View_Coord_FlipY | "Flip/Reflect Y (Latitude)" | check | — |
| (separator) | | | |
| N_View_Coord_ShiftX | "Shift X (Longitude)" | — | — |
| N_View_Coord_ShiftY | "Shift Y (Latitude)" | — | — |
| (separator) | | | |
| N_View_Coord_Speed | "Calculate Speed and Heading Only by Coordinates" | check | — |

**View ▸ Current Coordinates** (`N_View_UseCoordinates`, "Current Coordinates")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_MarkerURLSetup | "Setup URL for Command" | — | — |
| N_View_MarkerURLOpen | "Execute Command for Current Point" | — | 77 = **M** (bare key) |
| (separator) | | | |
| N_View_Coord_CopySetup | "Setup Template for Copy" | — | — |
| N_View_Coord_Copy | "Copy Current Point to Clipboard" | — | 67 = **C** (bare key) |

**View ▸ Panels Refresh** (`N_View_Refresh`, "Panels Refresh")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Refresh_Slow | "Slow" | radio | — |
| N_View_Refresh_Norm | "Normal" | radio | — |
| N_View_Refresh_Fast | "Fast" | radio, **checked** | — |

**View ▸ Measurement** (`N_View_Measurement`, "Measurement")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Measu_Metric | "Metric system (meters, kilometers)" | radio, **checked** | — |
| N_View_Measu_British | "British system (feet, miles)" | radio | — |

**View ▸ Time Zone** (`N_View_TimeZone`, "Time Zone")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_TimeZone_Local | "Local System (+N)" (GM-093) | radio, **checked** | — |
| (separator) | | | |
| 25 generated items "GMT-12" … "GMT+12" | | radio | — |

**View ▸ Graphics** (`N_View_Graphics`, "Graphics")

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Graphics_S | "Speed" | **checked** | 49235 = Ctrl+Alt+S |
| N_View_Graphics_H | "Altitude" | — | 49224 = Ctrl+Alt+H |
| N_View_Graphics_G | "G-Sensor Value" | **checked** | 49223 = Ctrl+Alt+G |
| N_View_Graphics_V | "Acceleration Total Vector" | — | 49238 = Ctrl+Alt+V |
| N_View_Graphics_X | "Acceleration Left-Right" | — | 49240 = Ctrl+Alt+X |
| N_View_Graphics_Y | "Acceleration Forward-Backward" | — | 49241 = Ctrl+Alt+Y |
| N_View_Graphics_Z | "Acceleration Up-Down" | — | 49242 = Ctrl+Alt+Z |
| (separator) | | | |
| N_View_Graphics_Hide | "Hide All Graphics" | — | 117 = F6 |
| N_View_Graphics_Show | "Show All Graphics" | — | 118 = F7 |

**View ▸ Timeline** (`N_View_Time`, "Timeline"). Mostly another spec; listed because the items appear in `PopupMenuGraphics`.

| Item | English | DFM | Shortcut |
|---|---|---|---|
| N_View_Time_ZoomIn | "Zoom In" | — | 16571 = Ctrl+VK_OEM_PLUS |
| N_View_Time_ZoomOut | "Zoom Out" | — | 16573 = Ctrl+VK_OEM_MINUS |
| N_View_Movie_Overlap | "Set video files overlap time" | — | 16458 = Ctrl+J |
| N_View_Time_SetA | "Set selection start (A) in current position" | — | 16420 = Ctrl+Home |
| N_View_Time_SetB | "Set selection end (B) in current position" | — | 16419 = Ctrl+End |
| N_View_Time_LoopAB | "Loop A-B" | — | 16460 = Ctrl+L |

**File**

| Item | English | Shortcut |
|---|---|---|
| N_File_Convert_GPS | "Convert GPS Log" (opens TFormGPSLog) | 16455 = Ctrl+G |

### 2.3 Colour keys (TRANSLATE_EN.TXT defaults; per-language overridable)

| Key | Default | Used for |
|---|---|---|
| colorMainBG | clBtnFace | Form background, gauge backgrounds, seek-bar background |
| colorGraphBG | #FFFFFF | Graph background; Details mini-graph |
| colorGraphBorderT | #666666 | 1-px top line of each graph |
| colorGraphBorderB | #CCCCCC | 1-px bottom line of each graph |
| colorGraphText | #000000 | Axis labels, peak labels, peak "+" marks |
| colorGraphOS | #000000 | Axes and ticks ("OS" = Russian *оси*, "axes") |
| colorGraphOSm | #EEEEEE | Horizontal grid lines |
| colorGraph_S | #0000FF | Speed trace |
| colorGraph_H | #000000 | Altitude trace |
| colorGraph_G | #996E0B | "G-Sensor Value" trace **and** total-vector (V) trace |
| colorGraph_X | #FF0000 | X trace |
| colorGraph_Y | #008000 | Y trace |
| colorGraph_Z | #D300A9 | Z trace |
| colorSpeedArrow | #FF0000 | Speed-gauge needle (fill and outline) |
| colorSpeedBG | #FFFFFF | Speed-gauge value boxes |
| colorSpeedText | #000000 | Speed-gauge box outline |
| gmStartX / gmStartY / gmStartZoom | −74.005767 / 40.713003 / 14 | Initial map centre (lon/lat) and zoom when nothing is saved (EN = New York) |

---

## 3. Requirements

### 3.1 Settings persistence and menu mechanics

**GM-001 Registry store.**
- All settings are string values under `\Software\DATAKAM PLAYER\` (UnitReg: initialization L152–L153). The hive is presumably HKCU (see Q1).
- Booleans are stored as the strings `"true"`/`"false"`. Any other content reads as the supplied default (UnitReg: regReadB L142–L149; regWriteB L112–L116).
- Floats are written with 3 fixed decimals and "." as the decimal separator.
- The Rust app may use any store but must keep the same names and defaults.

**GM-002 Menu check persistence.** Every checkable menu item persists under its component name, e.g. `N_View_Graphics_S` or `N_View_Arrow_Big`.
- On start-up, the DFM checked set is captured as the defaults (ReadDafaultsMenu L577–L583). Then every item's Checked is read from the registry, falling back to its DFM state (ReadRegMenu L587–L593).
- On change, a value equal to the DFM default is **deleted**; otherwise it is written (UnitFormMain: DoReg L2690–L2695).
- For radio items, all siblings in the same parent are saved (MenuItemSaveReg L2696–L2707).

**GM-003 Generic click semantics.**
- `MenuItemClick`: a radio item becomes checked; a non-radio item toggles. The state is then mirrored to its popup-menu twin and persisted (UnitFormMain: TFormMain.MenuItemClick L2710–L2721).
- `MenuItemSet(item, value)` does the same only when the value actually changes (L2724–L2730).
- Every handler below starts with this step unless stated otherwise.
- Handlers called with Sender = nil (at start-up) skip the toggle and only apply the current state.

**GM-004 Video-window popup menu.** `PopupMenuVideoWindow` is built at start-up as a mirror of the main menu (CopyMainMenu L631–L643):
1. all Playback items except Splitters, Decoders and Renderer;
2. a separator;
3. the File, View and Zoom top-level menus;
4. a separator;
5. Exit.

Mirrored items share caption, check state, radio flag, shortcut and click handler (MenuItemCopy L2755–L2767).

**GM-005 Graphics popup menu.** `PopupMenuGraphics` (right-click on the seek bar or any graph) contains, in order (CopyMainMenu L644–L652):
1. Timeline ▸ Zoom In, Zoom Out, Set overlap, Set A, Set B (everything except Loop A-B);
2. File ▸ "Save selected fragment" (`N_File_Save_Video`);
3. a separator;
4. all Graphics items (S, H, G, V, X, Y, Z, separator, Hide, Show);
5. a separator;
6. Details.

Opening it hides the Details popup (UnitFormMain: TFormMain.PopupMenuGraphicsPopup L4995–L4997).

### 3.2 Main window, splitters and panel toggles

**GM-010 Initial window geometry** (UnitFormMain: TFormMain.FormCreate L975–L1024).
1. Default size: width = 7/8 and height = 3/4 of the primary monitor, centred on it.
2. Override from `pos_Width`, `pos_Height`, `pos_Top`, `pos_Left` (integers) when present.
3. Clamp to the work area of the window's monitor:
   - if Left > workRight − 100 or Left + Width < workLeft + 100, set Left = workLeft;
   - if Top > workBottom − 100 or Top < workTop, set Top = workTop.
4. Window state from `pos_State`, default **2 = maximized**.
5. Caption is "DATAKAM PLAYER - Registrator Viewer" (FormCreate L1007; DoTranslate L4712–L4716).

**GM-011 Saved panel sizes on start-up** (FormCreate L972, L1013–L1015).
- `PanelRight.Width` = `panel_Right` (default: DFM 510), minimum 5.
- `PanelBottom.Height` = `panel_Bottom` (default: DFM 185).
- Map height fraction `BrowserH` = `panel_Browser` (float, default **0.6**).
- The ads and analytics browsers get height 0 and are made invisible (L1000–L1003).

**GM-012 Start-up handler order** (FormCreate L986–L998). After the registry menu state is loaded, these run with Sender = nil, in this order:
1. Refresh rate (GM-090);
2. Measurement (GM-091);
3. Time zone (GM-093);
4. FileList/panels/map (GM-016);
5. Graphics (GM-030);
6. Profile (GM-020);
7. Coordinate flags (GM-180).

**GM-013 Window position persistence.** On every position/size change while visible (UnitFormMain: TFormMain.WMWindowPosChanged L5002–L5015):
- write `pos_State` unless minimized;
- write `pos_Left`, `pos_Top`, `pos_Width`, `pos_Height` from the *normal* (restored) rectangle.

**GM-014 Splitter persistence.** `SplitterRight` and `SplitterBottom` share one handler (UnitFormMain: TFormMain.SplitterRightMoved L2524–L2533). When moved by the user it:
1. re-runs FormResize;
2. writes `panel_Right` (only if PanelRight.Width > 0);
3. writes `panel_Bottom`;
4. writes `panel_Browser` (3 decimals).

**GM-015 Map/list splitter** (UnitFormMain: TFormMain.SplitterListViewMoved L2515–L2521). Applies only when PanelRight2 is taller than 20 px and the map is visible and top- or bottom-aligned:
- `BrowserH` = WebBrowser.Height / PanelRight2.Height;
- on a user move, call GM-014 to persist.

The map always keeps this *fraction* of the column height on resize (GM-019).

**GM-016 View ▸ File List / Control Panel / Details** (F2 / F4 / F8). All three, and every map-provider item, share `N_View_FileListClick` (UnitFormMain L2999–L3096).
1. Toggle and persist (GM-003).
2. Map-provider part: see GM-150.
3. `PanelListView.Visible` = File List checked. `PanelControl.Visible` = Control Panel checked.
4. `SplitterListView.Visible` = list visible **and** map visible.
5. If Details is unchecked, hide the Details popup.
6. The right column has content if the list is visible, the map is visible, or the control panel is visible (see Q4).
   - No content: `PanelRight.Width` = 0.
   - Content and width < 20: width = 510.
   - `SplitterRight.Width` = 7 (SplitterBottom's height) when there is content, else 0.
7. List visible:
   - if the map is visible with height 0, give it height 10;
   - map Align = alTop; `SplitterListView` aligned alTop directly below the map;
   - `PanelListView` = alClient.
8. List hidden:
   - list and splitter Align = none (hidden);
   - map Align = alClient (fills the column) if visible.
9. The analytics browser stays alTop at Top 0, height 0.
10. When invoked by the user: FormResize, then GM-014 persistence.

**GM-017 Right-column composition**, derived from GM-016 and GM-020.

GPS profiles, top to bottom:
1. map (height = round(BrowserH × PanelRight2.Height));
2. 7-px splitter;
3. file list (fills).

Video profile, top to bottom:
1. map (hidden; provider = None);
2. file list (fills);
3. control-panel strip (56 px) at the bottom.

**GM-018 Window resize** (UnitFormMain: TFormMain.FormResize L1485–L1531). Runs only while the form is visible and the application is not terminating.
1. If PanelControl is visible, run AlignControlPanel (GM-021).
2. Re-apply the video zoom (other spec).
3. Recompute the timeline selections and re-prepare the seek bar (GM-061).
4. Re-stack and redraw all graphs (GM-031).
5. If the map is visible and top/bottom aligned: map height = round(PanelRight2.Height × BrowserH) (L1520–L1521; operands confirmed in disassembly).
6. Zoom bar layout: `ButtonZoomIn` at the right edge of `PanelZoomScroll` (left = clientWidth − button width); `ScrollBarZoom` fills between the two buttons (L1523–L1524).
7. Call the timer handler once with Sender = nil (redraws cursors and gauges, no map recentring) (L1526).
8. If the full-screen form is showing, re-layout it too (L1528–L1529; other spec).

**GM-019 Map height follows the column.** Because of GM-018 step 5, resizing the window keeps the map at the same fraction of `PanelRight2`. Only GM-015 changes the fraction.

### 3.3 Layout profiles and the control panel

**GM-020 Profile switch.** Handler `N_View_Profile_GPSClick`, shared by the three radio items (UnitFormMain L5454–L5530).
1. Toggle and persist.
2. Gauge visibility:
   - `ImageGSensor` visible only in GPS_G;
   - `ImageSpeed` visible unless Video;
   - `ImageAzimut` follows `ImageSpeed`.
3. File-list columns with ID 3 (track, "km") and ID 4 (speed, "km/h"):
   - Video: width 1 (effectively hidden);
   - otherwise: if narrower than 2 px, restore 54 and 30 px.
4. Control-panel placement:
   - **GPS profiles:** `PanelGraphicsAddon.Width` = 260; `PanelControl` re-parented into `PanelGraphicsAddon` with Align = alClient; the ads browser goes to alBottom.
   - **Video:** `PanelGraphicsAddon.Width` = 0; `PanelControl` re-parented back into `PanelRight` with Align = alBottom; the ads browser goes to alTop.
5. Only on a user click (Sender ≠ nil):
   - Apply the profile to the Details popup (GM-121).
   - Map provider: Video → None; GPS or GPS_G → **Google Maps** (see Q2).
   - Graphs:
     - Video: "Hide all graphics" (GM-033).
     - Otherwise: Speed on; G-Sensor Value on only for GPS_G. For GPS, also force V, X, Y and Z off. Then rebuild the graphs (GM-030).
   - Re-run GM-016 (Sender nil), FormResize, persist splitters, refresh the list.
6. Menu enable states:
   - `N_View_Maps` is enabled unless Video.
   - Arrow, Time Points and Graphics are enabled when Maps is enabled.
   - Fix Coordinates and Measurement are enabled unless Video.
7. Inside the Graphics menu:
   - G, V, X, Y, Z are *visible* only in GPS_G;
   - every other entry (S, H, separator, Hide, Show) is visible when Maps is enabled.

**GM-021 Control-panel flow layout** (UnitFormMain: TFormMain.AlignControlPanel L1377–L1478). The controls are laid out in this fixed order:
1. `PanelControl_s1` (105×48)
2. `PanelPlaySpeed` (108×17)
3. `PanelControl_s2` (105×23)
4. `TrackBarVolume` (113×24)
5. `CheckBoxLoop` (105×17)

followed by the *visible* gauges: `ImageSpeed` (134×100), `ImageGSensor` (65×65), `ImageAzimut` (57×57).

Parameters depend on PanelControl's alignment:
- **Strip mode** (alTop or alBottom, i.e. the Video profile): x0 = 4, y0 = 4. Panel height:
  - `PanelControl_s1.Height` + 8 (= 56) if ImageSpeed is hidden;
  - otherwise 60 if the panel is at least 621 px wide, 106 if at least 431, else 135.
- **Column mode** (alClient, the GPS profiles): x0 = 4, y0 = 0. The panel height is whatever PanelBottom gives it.
- In both modes dx = 8, dy = 4.

Algorithm (the tunable factor 1.1 is an extended constant):
1. Column width w starts as control 0's width; column left l starts as control 0's current Left.
2. Control 0 goes to (x0, y0).
3. For each next control c (previous control p):
   - If p.Width + c.Width ≥ 1.1 × w (too wide to sit side by side):
     - if c fits below p (p.Bottom + dy + c.Height ≤ panel height), place it at (l, p.Bottom + dy);
     - otherwise start a new column: l = l + w + dx, w = 0, place c at (l, y0).
   - Otherwise place c to the right of p: (p.Right + dx, p.Top).
   - A horizontal `TrackBarVolume` is additionally shifted by (−4, −3).
   - If c would extend past the panel bottom, move it up so its bottom = panel height.
   - Update w = max(w, c.Right − l).
4. In column mode, finally set `PanelGraphicsAddon.Width` = min(l + w, PanelBottom.Width / 2).

Worked example (GPS_G, PanelBottom height 185):
- Column 1, at x = 4 (volume at x = 0):

  | Control | y |
  |---|---|
  | buttons | 0 |
  | play speed | 52 |
  | frame buttons | 73 |
  | volume | 97 |
  | loop checkbox | 125 |

- Column 2: speed gauge at (121, 0); G-sensor at (121, 104); compass at (194, 104).
- Addon width = min(255, PanelBottom.Width / 2).
- In GPS (no G-sensor) the compass goes to (121, 104).

Worked example (Video, strip 56 px):
- buttons (4, 4);
- play speed (117, 4) and frame buttons (117, 25);
- volume (229, 1) and loop (233, 29).

**GM-022 Control-panel hints** (UnitFormMain: TFormMain.DoTranslate L4691–L4703). Lines are separated by CRLF.

| Control | Hint |
|---|---|
| Volume | "Volume" |
| PanelPlaySpeed | "Play Speed" |
| CheckBoxLoop | Caption "Loop A-B", hint "Loop selected path A-B" |
| ImageAzimut | "Bearing" |
| ImageGSensor | "x - Acceleration Left-Right" / "y - Acceleration Forward-Backward" / "z - Acceleration Up-Down" |
| ImageSpeed (metric) | "- Altitude, m" / "- Track Length, m" / "- Speed, km/h" |
| ImageSpeed (British) | "- Altitude, feet" / "- Track Length, miles" / "- Speed, mph" |

The three ImageSpeed lines match the gauge's three boxes from top to bottom.

### 3.4 Graphs

**GM-030 Graph set rebuild.** Handler `N_View_Graphics_SClick`, shared by S/H/G/V/X/Y/Z (UnitFormMain L2929–L2968).
1. Toggle and persist.
2. Destroy all existing graphs and their images.
3. Create one graph per checked item, in the fixed order **S, H, G, V, X, Y, Z**. Each image:
   - is a child of `PanelBottomGraphics`;
   - uses the seek-bar mouse handlers;
   - hides the Details popup on mouse leave;
   - uses `PopupMenuGraphics` (NewImage L2911–L2922).
4. Each graph's horizontal border equals the seek bar's border, so pixel columns align exactly with the seek bar.
5. `SplitterBottom.Visible` = at least one graph enabled.
6. Only on a user action (Sender ≠ nil), adjust `PanelBottom.Height`, then persist (GM-014) and FormResize:
   - no graphs: height = ImageSeek.Height + PanelZoomScroll.Height (= 57);
   - all seven graphs and height ≤ 57 + 150: height = 320;
   - some graphs and height < 186: height = 185.

**GM-031 Vertical stacking** (UnitFormMain: TFormMain.FormResize L1500–L1517).
- free = PanelBottomGraphics.Height − ImageSeek.Bottom.
- h = max(free div max(1, count), 5) (see Q16).
- Graph i: Top = ImageSeek.Bottom + i × h, Left = 0, Width = PanelBottomGraphics.Width, Height = h. The last graph takes the remainder, max(PanelBottomGraphics.Height − Top, 5).
- There are no gaps between graphs.
- On every FormResize, each graph is:
  1. resized;
  2. cleared to colorGraphBG;
  3. data-filled (GM-034);
  4. given its top/bottom lines (GM-039);
  5. if no file is loaded, shown without a cursor.

**GM-032 Graph classes** (UnitGraphics: TSpeedGraphic … TGsensorZGraphic Create/GetName/GetTitle/GetGraphicFunction L339–L502).

| Menu | Title letter | Axis step `osDelta` | Colour key | Source series (GM-104) |
|---|---|---|---|---|
| S | "S" | 10 | colorGraph_S | GPS speed, km/h |
| H | "H" | 5 | colorGraph_H | GPS altitude, m |
| G | "G" | 0.1 | colorGraph_G | G-sensor "G" value |
| V | "V" | 0.1 | colorGraph_G | G-sensor total vector |
| X | "X" | 0.1 | colorGraph_X | G-sensor X |
| Y | "Y" | 0.1 | colorGraph_Y | G-sensor Y |
| Z | "Z" | 0.1 | colorGraph_Z | G-sensor Z |

**GM-033 Show/Hide all** (UnitFormMain: TFormMain.N_View_Graphics_ShowClick L2973–L2986), F7 / F6.
1. b = true for Show, false for Hide.
2. Set S and H to b.
3. If the G item is not visible (non-GPS_G profile), force b = false for the rest.
4. Set G, V, X, Y and Z to b.
5. Rebuild as a user action (GM-030, including the height rules) and persist splitters.

**GM-034 Per-column data binning** (UnitFormMain: TFormMain.DrawBackAuto L2485–L2511; UnitMovingInfo: TMovingInfo.ExportGraphic L781–L819, AddValue L770–L780).
- Column count N = max(ImageSeek.Width − 2 × seekBorder, 0), identical for the seek bar and every graph. Skip if N = 0 or zoomL ≥ zoomR.
- time1/time2 = absolute times of the visible window edges zoomL/zoomR.
- Every sub-file of the current group that overlaps the window contributes to the same array of N bins {min, max, count, time-of-first-sample}.
- Bin width k = (time2 − time1)/(N − 1); k = 4 × span when N = 1. A sample at time t ≥ time1 goes to bin round((t − time1)/k).
- Edge handling:
  - the value linearly interpolated at time1 is added to bin 0 when the previous sample lies before time1;
  - at the first sample past time2, the value interpolated at time2 goes into the last bin (only if that bin is empty), and processing stops.
- Empty bins (count 0) are gaps.
- British units: speed values × 0.621371192, altitude × 3.280839895. G series are unchanged, and `osDelta` is not converted (steps become 10 mph and 5 ft).

**GM-035 Scale** (UnitGraphics: TMyGraphic.DrawBack L127–L154).
- Altitude only, before scaling: in each non-empty bin, a min of 0 is replaced by the max, and a max of 0 marks the bin empty. Zero altitude means "no data".
- maxV = the largest |min| or |max| over all bins, but at least 1.05 × osDelta.
- `positiveOnly` = no bin from index 1 onward has min < 0.
- Zero line y0:
  - positiveOnly: Height − borderB (zero at the bottom);
  - otherwise: (Height + borderT − borderB) div 2 (symmetric scale).
- Value to pixel: y = y0 − round(v × (y0 − borderT) / maxV) (yConv L120–L122).
- borderT = text height of "9" if the image is ≥ 60 px tall, else textHeight div 2 + 1; borderB = textHeight div 2 + 1 (TMyGraphic.Resized L103–L111).
- Horizontal border borderX = seek border (GM-060).

**GM-036 Background bands.** Before the trace, the same locked/error/A-B/file-separator bands as the seek bar are painted over the full graph height (GM-063).

**GM-037 Axes and grid** (UnitGraphics: TMyGraphic.DrawOs L255–L294).
- Lines in colorGraphOS:
  - vertical lines at x = borderX and x = Width − borderX, full height;
  - zero line at y0 between them.
- Grid step d: start at osDelta and add osDelta until the pixel spacing (y0 − borderT)/(maxV/d) ≥ 1.2 × text height. The cap is 1000 × osDelta.
- For each multiple of d below maxV, both positive and negative, when inside [borderT, Height − borderB]:
  - 3-px tick across the left axis;
  - label (value as a plain float string) right-aligned 2 px left of the axis, vertically centred;
  - grid line in colorGraphOSm across the plot, except on the zero line.
- The title letter (GM-032) is centred in the right margin, vertically centred on the plot area.
- Graphs have no time axis.

**GM-038 Data trace** (UnitGraphics: TMyGraphic.DrawBack L158–L195). Pen is the graph colour. For each non-empty bin i at x = borderX + i:
- Vertical bar from y(max) to y(min) (1 px if equal).
- Connector to the previous drawn bin, unless broken. The link is broken when (i − last ≥ 4 and the bins' times differ by > 11 s) or (bin time = 0 and i − last > 50).
  - Previous bin entirely below: line from its top to this bin's bottom.
  - This bin entirely below: line from the previous bin's bottom to this bin's top.
  - Ranges overlap: top-to-top line, only if bins were skipped.

**GM-039 Peak labels** (UnitGraphics: TMyGraphic.DrawBack L197–L245). Up to 5 times:
1. Pick the non-empty bin whose dominant value lv has the largest |lv|. lv = max, or min when |min| > max.
2. Draw a small "+" in colorGraphText at the point.
3. Draw label = lv rounded to osDelta/10 (integer for speed, 0.5 for altitude, 0.01 for G):
   - transparent background, at x + 2;
   - above the point for lv ≥ 0 (not above y = 1); below it for lv < 0 (kept inside the image).
4. Mark the peak's "hill" as used: walk right, then left, emptying bins until at least 2 × labelWidth + 10 columns away *and* the trace starts rising again.

This happens after the trace is drawn, so only the label search is affected.

**GM-040 Top/bottom lines** (UnitGraphics: TMyGraphic.DrawTopAndBottom L299–L308): 1-px line at y = 0 in colorGraphBorderT and at y = Height − 1 in colorGraphBorderB.

**GM-041 Graph cursor** (UnitGraphics: TMyGraphic.Draw L313–L334), each timer tick (TimerTimer L1357–L1358).
- Copy the cached background.
- If the play position p is inside [zoomL, zoomR]: x = borderX + round((p − zoomL)/(zoomR − zoomL) × (Width − 2 × borderX)).
- Draw a full-height vertical line using a red pen in raster mode NOT-MERGE: pixel = NOT(pen OR screen). This shows black on white (see Q7).

### 3.5 Seek bar

**GM-060 Seek-bar geometry** (UnitBitmaps: TSeekBitmap.Create L401–L408; UnitFormMain: ReplaceSeekImage L687–L700).
- Font size 8.
- border = TextWidth("99:99:99") div 2 + 4.
- Pin size pinH = 10.
- ImageSeek is 40 px tall, alTop, background colorMainBG.

**GM-061 Prepare** (UnitBitmaps: TSeekBitmap.Prepare L417–L489; called by FormResize L1494–L1498 with the group's absolute start/end plus the time-zone shift, and zoomL/zoomR).
1. Clear to colorMainBG; draw the bands (GM-063).
2. Visible absolute times t1, t2. "Seconds mode" when t2 − t1 < 4 min.
3. Both tick rows share these rules:
   - Base step: 1 s in seconds mode, otherwise 1 min. The step grows by the base step until the tick count ≤ trunc(w / (TextWidth(template) + 20)), where w = Width − 2 × border.
   - A label is drawn only if it fits since the previous drawn label.
4. **Upper row: time since group start.**
   - Template "99:99" (seconds) or "99".
   - Label format `TimeToStrSmall`:
     - "dd" + "d" + "hh:mm" when days > 0;
     - "h:mm" when hours > 0;
     - else "m";
     - plus ":ss" in seconds mode.
   - Text sits above the groove; the tick runs from mid-height up by pinH/2 + 2.
5. **Lower row: absolute clock time.**
   - Template "99:99:99", ticks aligned to whole steps of the day.
   - Label = locale long time.
   - Text below the groove; the tick runs down from mid-height.
6. **Groove:** rounded rect from (border − 5, H/2 − 5) to (W − border + 5, H/2 + 5), corner radius 9, black pen, `clBtnShadow` fill.

**GM-062 Seek-bar pin** (UnitBitmaps: TSeekBitmap.Draw L500–L545), each timer tick (TimerTimer L1355–L1356).
1. Blit the background.
2. If the position is visible, at x:
   - black vertical line from y = H/2 − 10 to H/2 + 10;
   - **top bubble**: white rounded rect (radius 5, pen RGB(100,100,100)) with the elapsed time `TimeToStrSmall` with seconds;
   - **bottom bubble**: same style, with the absolute clock time (locale long time);
   - **pin**: 10-px circle, black pen, `clBtnHighlight` fill.
3. Fraction digits on the elapsed time depend on the time per pixel (perPx = visible span / columns):
   - perPx < 0.01 s: ".mmm";
   - perPx < 0.1 s: ".d" (tenths);
   - otherwise none.

**GM-063 Bands** (UnitFormMain: TFormMain.CalculateCurrentSelections L2432–L2477; UnitBitmaps: D L100–L118, DrawDelections L119–L124).
- Bands cover the full height, with columns offset by border.
- A pair (x1, x2) with x2 = x1 + 1 is drawn as a 1-px line; otherwise as a filled rectangle.
- Columns for a sub-file are xs = floor(N × (start − zoomL)/span) and xe = ceil(N × (end − zoomL)/span).
- Drawn in this order:

  | # | Band | Fill | Line |
  |---|---|---|---|
  | 1 | Locked files (lock flag set) | RGB(255,255,180) | same |
  | 2 | Error files (status 2) | RGB(240,240,0) | same |
  | 3 | A–B selection (selectL..selectR, when selectL < selectR and overlapping) | `clHighlight` | same |
  | 4 | Areas without video (before the first file, between files, after the last file) | RGB(200,200,200) | RGB(100,100,100) if the separator list has fewer than 5 entries, else RGB(200,200,200) |

**GM-064 Seek mouse interaction.** The same handlers serve ImageSeek, every graph image, PanelBottom and PanelBottomGraphics. X is always mapped through the seek geometry (UnitFormMain: TFormMain.ImageSeekMouseDown L1915–L1927, ImageSeekMouseMove L1935–L1961, ImageSeekMouseUp L1967–L1972; UnitBitmaps: TSeekBitmap.GetProgress L551–L556).
- Progress at x = zoomL + clamp((x − border)/max(1, W − 2 × border), 0, 1) × (zoomR − zoomL).
- **Left/middle press** (right button ignored):
  - on ImageSeek within 4 px of the pin: start dragging the pin;
  - otherwise seek immediately.
- **Drag:** the displayed position follows the mouse and redraws at once. Release seeks.
- **Hover:** hand cursor within 4 px of the pin. The Details popup is driven by hover (GM-120).

### 3.6 Timeline zoom (shared by seek bar and graphs)

**GM-070 Zoom window state** (FormCreate L958–L962): zoomL = 0, zoomR = 1, zoom factor zoomK = 2, selectL = selectR = 0.

**GM-071 Zoom in/out** (UnitFormMain: TFormMain.ButtonZoomInClick L4006–L4020). Used by the "-"/"+" buttons, the Timeline menu items and their popup copies.
- width = |zoomR − zoomL|, divided (in) or multiplied (out) by zoomK.
- Recentred on the current play position.
- Then GM-072 and FormResize.

**GM-072 Validate the zoom bar** (UnitFormMain: TFormMain.SetupZoomBar L4040–L4084).
1. Swap if inverted.
2. Enforce width ≥ minZoom = 0.5 s / (1 s + group duration) (GetMinZoom L3999–L4001).
3. Shift the window, without shrinking it, into [0, 1].
4. Scrollbar values:
   - page = max(1, (ImageSeek.Width − 2 × border) div 10);
   - max = round(page/width) − 1;
   - position = round(max × zoomL).
5. The scrollbar is disabled when max < 2 or max ≤ page.
6. Then update the map A/B markers (GM-168).

**GM-073 Scroll the zoom bar** (UnitFormMain: TFormMain.ScrollBarZoomChange L4089–L4103).
- Enabled: zoomL = (pos − min)/(max − min), width kept. (Min stays 1 from the DFM; see Q8.)
- Disabled: whole range.
- Then GM-168 and FormResize.

**GM-074 Auto-scroll during playback** (UnitFormMain: TFormMain.TimerTimer L1258–L1286), on real timer ticks while the media graph is running:
- Loop A-B on and position ≥ selectR: seek to selectL.
- Else if position > zoomR − 0.05 × width and zoomR < 1: page forward so position sits 10 % from the left.
- Else if position < zoomL: centre on the position.
- Then GM-072 and FormResize.

**GM-075 Mouse wheel** (UnitFormMain: TFormMain.FormMouseWheel L5343–L5392). d = WheelDelta div 120; d = 0 is ignored. Routing is by the control under the cursor:

| Under cursor | Action |
|---|---|
| Video | Video zoom (other spec), factor 1.2 |
| `PanelBottomGraphics` | Timeline zoom with factor **1.5**: d > 0 zooms in, d < 0 zooms out. Factor reverts to 2 afterwards. |
| `PanelZoomScroll` | Scroll position −= 10 × d, clamped |
| `PanelControl` | Frame step when paused (d > 0 = previous frame); jump back/forward when playing |
| Map | GM-170 |

### 3.7 Values (TMovingInfo)

**GM-100 Units and records.**
- Internal units: time in days, speed in km/h, altitude and track distance in metres, coordinates in WGS84 degrees.
- GPS point: time, lon, lat, altitude (0 = unknown), bearing (−1 = unknown), speed (−1 = unknown), cumulative track metres, "warn" flag, index.
- G-sensor sample: time, x, y, z, V, G, linked GPS index, sample index.

**GM-101 Coordinate corrections at load** (UnitGPSManager: TGpsInfoManager.CorrectGPS L475–L495). Applied before any other processing, only when the main window exists:
1. negate longitude if FlipX is checked;
2. negate latitude if FlipY is checked;
3. then add `gm_shiftX` to longitude and `gm_shiftY` to latitude (degrees, default "0").

**GM-102 GPS clean-up** (UnitMovingInfo: TMovingInfo.CreateAutoSelectSourceInfo L165–L211; GPSDelSamePoints L254–L280; GPSFixJumps L287–L313; GPSFixSecondsRoundMistakes L320–L345; GPSMarkMistakedPoints L362–L410). Skipped if the source requests no post-processing. In order:
1. **Duplicates:** drop a point less than 0.9 s after the last kept point when it has a zero coordinate or moved by < 8.993e-8° in both axes.
2. **Date jumps:** fix whole-day jumps.
   - If the last point is more than 1 day off the 75th-percentile time, subtract the whole days from it.
   - A backward gap > 0.5 day adds round(gap) days to the earlier point.
   - A single out-of-order point is set to the mean of its neighbours.
3. **Rounded timestamps:** median interval m (1 s if m < 0.09 s). Points not at least m/20 apart are pushed back in steps of m, walking backwards.
4. **Spikes:** pairwise speed is computed with the fast distance (GM-176).
   - Speed > 500 km/h to the next point: next point is marked bad and removed.
   - If the speed over 3 steps > 5 km/h and > 8 × the middle step's speed, the worse neighbour is removed.
   - If (s23 + s01) × 4 < s12, point i+2 is marked "warn".

**GM-103 G-sensor timing and normalisation** (UnitMovingInfo: ExtropaleteGSensorTimes L459–L571; NormalizeGSensor L744–L750; BuildGSensor L755–L762).
1. Samples without time take the time of their linked GPS point. Duplicate and non-monotonic times are cleared.
2. If no sample has a time, they are spread between the file's creation and write times. If that span is ≤ 0 or > 4 h, use write time − 0.1015 s × count.
3. Untimed samples are interpolated, and the ends extrapolated at 0.99 × median interval.
4. If the G and GPS time ranges overlap only partially, all G times are shifted by the smaller of (GPS end − G end) and (GPS start − G start).
5. **Normalisation:** subtract the series median from x, y and z separately. Values become dynamic accelerations around 0 (gravity and mounting bias removed).
6. **V** ("Acceleration Total Vector") = √(x² + y² + z²).
7. **G** ("G-Sensor Value") = √((x_i − x_{i+1})² + (y_i − y_{i+1})²): horizontal change to the next sample, z ignored.
8. The last sample keeps V = G = 0.

**GM-104 Derived GPS values** (UnitMovingInfo: TMovingInfo.CalculateEmptyDatas L1346–L1525).
- **Coordinate-only mode:** if "Calculate Speed and Heading Only by Coordinates" is checked, every point's speed and bearing are set to unknown and the warn flags cleared first.
- **Drop** points with an integer-degree longitude or latitude (this includes zeroed points), and warned points that duplicate the previous position.
- **Track distance:** cumulative great-circle distance. A step adds only when its speed < 500 m/s, its length < 1000 m and its time gap < 20 s. Warned points add nothing, so GPS gaps do not add distance.
- **Altitude:** a 0 altitude at the first or last point copies its neighbour.
- **Missing bearing:** atan2(Δlon × cos(lat), Δlat) between points i−2 and i+4 (clamped to the ends), in [0, 360), 0 = north, clockwise.
- **Missing speed:** centred window ±n (n = 1..10, growing until it spans a time different from point i):
  - speed = Δtrack / Δtime in km/h;
  - aborts at warned points.
- **Gaps between valid speeds:** speed and track linearly interpolated by index; bearing set to unknown.
- Remaining unknown speeds become 0.
- Unknown bearings take the first known one (start) or the previous one.

**GM-105 Value at an instant** (UnitMovingInfo: TMovingInfo.Get L592–L687; UnitFormMain: GetMovingInfo L2206–L2215). Used by the gauges, Details, the map arrow and the coordinate templates.
- **G-sensor:** linear interpolation of all fields between the bracketing samples. Within 5.5 s outside the range, the end sample is used; further out the result is empty.
- **GPS:** linear interpolation of time, lon, lat, speed and track.
  - Altitude: if one side is 0, take the other side.
  - Bearing: interpolated on the shorter arc, with −1 on one side taking the other side.
  - Index and warn flag come from the left point.
- **gpsGood** = both neighbours not warned AND (gap < 11 s OR the instant is < 2.75 s from one neighbour).
- Within 5.5 s outside the track, the end point is used. Further out the result is empty (time 0).
- There is no further smoothing.

**GM-106 Track statistics** (UnitMovingInfo: GetTrackLength L995–L999; GetAvgSpeed L1004–L1011).
- Length = last − first cumulative metres.
- Average speed = length / duration in km/h; −1 if fewer than 2 points.

### 3.8 Gauges

**GM-080 Gauge update rule** (UnitFormMain: TFormMain.TimerTimer L1291–L1351). Each tick, only when the main form is the active window (or the full-screen form hosts the bottom panel) and the profile is not Video:
1. Fetch the value at the current position (GM-105).
2. If the GPS time > 0 and the control panel is visible: redraw Speed and Azimuth.
   - Metric: speed km/h, track m, altitude m.
   - British: speed / 1.609344 (mph), track / 1000 / 1.609344 (miles, 3 decimals), altitude / 0.3048 (feet).
3. If the G-sensor time > 0 and the control panel is visible: redraw the G-sensor gauge with x, y, z.
4. With no data the gauges keep their last image (Q5). ClearAll resets them to the initial state:
   - Speed: no needle, empty boxes;
   - Azimuth: no needle;
   - G-sensor: markers off-image.

**GM-081 Speed gauge** (UnitBitmaps: TSpeedBitmap.Create L141–L154, Draw L166–L215). Image 134×100.
- **Background:** the resource bitmap `FORM_MAIN_SPEED.BMP` — a pre-drawn half dial 0–200 in steps of 20. Positions: 0 at the left, 100 at the top, 180 at the right, 200 about 20° below the right.
  - The four corners are flood-filled with colorMainBG.
  - The dial artwork is the same in mph mode.
- **Needle** (only if speed ≥ 0): angle = 180° + speed (1° per unit, unclamped). Pivot (67, 67).
  - Filled triangle in colorSpeedArrow.
  - Tip at radius 60. The two base corners are ±6 px perpendicular, mirrored through the pivot.
- **Three rounded boxes**, pen colorSpeedText, fill colorSpeedBG, centre x = 67:

  | Box | Rect | Radius | Text |
  |---|---|---|---|
  | Speed (bottom) | (37, 56)–(97, 95) | 10 | round(speed); Arial 22; centred; vertical centre y = 76 |
  | Track (middle) | (42, 40)–(92, 54) | 5 | Arial 8, right-aligned at x = 88, centre y = 47 |
  | Altitude (top) | (50, 26)–(84, 38) | 5 | round(altitude) only if ≠ 0; Arial 8; centred at y = 32 |

  - Track text metric: integer metres with a space before the last 3 digits when 4–6 characters long ("1 234", "123 456").
  - Track text British: miles with 3 decimals.

**GM-082 Compass gauge** (UnitBitmaps: TAzimutBitmap.Create L223–L250, Draw L256–L304). Image 57×57.
- **Background:** colorMainBG; white disc with a grey RGB(150,150,150) 2-px rim and a black 1-px outline. 4-px black ticks at the left, right and bottom (none at the top).
- **Needle** (only if bearing ≥ 0), in heading-up orientation. r = 27.5, half-width ≈ 7.
  - Red half (fill red, outline RGB(255,100,100)) toward (sin a, cos a) from the centre.
  - Blue half (fill blue, outline RGB(80,80,255)) opposite.
  - Heading 0 puts blue up; heading 90 puts blue to the left. The blue half points to north relative to a vehicle facing up.
- **Bearing label:** white rounded box (W/2 + 2, H/4 − 3)–(W/2 + 19, H/4 + 7) with round(bearing), Arial 6.
- **Always drawn:** a line from the centre to the top, a small filled triangle at the top (heading mark) and a black 5-px hub dot.

**GM-083 G-sensor gauge** (UnitBitmaps: TGSensorBitmap.Create L311–L382, Draw L389–L396). Image 65×65.
- oneG = W div 2 − 10 = 22 px; oneGY = round(oneG / 1.4) = 16 px.
- Three black axes, each ±(1 g + 5 px) with arrowheads at both ends, a 7-px cross at the origin, ticks at ±1 g and an Arial 8 label:

  | Axis | Origin | Direction | Label position |
  |---|---|---|---|
  | X | (29, 60) | horizontal along the bottom | "x" above the origin |
  | Z | (60, 29) | vertical along the right | "z" left of the origin |
  | Y | (25, 25) | diagonal lower-left ↔ upper-right | "y" upper-left of the origin |

- Draw(x, y, z) puts a 5-px white circle with a black outline on each axis:

  | Axis | Marker position | Direction of positive value |
  |---|---|---|
  | X | (29 − 22x, 60) | left |
  | Z | (60, 29 + 22z) | down |
  | Y | (25 − 16y, 25 + 16y) | down-left |

- There is no trail.

### 3.9 Refresh, measurement units and time zone

**GM-090 Refresh rate** (UnitFormMain: TFormMain.N_View_Refresh_SlowClick L3568–L3580). Timer interval:

| Setting | Interval |
|---|---|
| Slow | 1000 ms |
| Normal | 300 ms |
| Fast (default) | 100 ms; **200 ms** when Yandex Maps 2.0 is the provider |

Every tick redraws the seek pin, graph cursors, gauges, map arrow and Details mini-graph cursor.

**GM-091 Measurement units** (UnitFormMain: TFormMain.N_View_Measu_MetricClick L5405–L5412).
- A global British flag = British item checked.
- On a user click: re-translate (hints, list column caption "km/h"/"mph", Details unit strings), then FormResize (graphs rescaled).
- Conversion factors:
  - km → miles: ÷ 1.609344;
  - m → feet: ÷ 0.3048;
  - graphs: × 0.621371192 and × 3.280839895.

**GM-092 Time-zone menu** (UnitFormMain: AddTimeZones L551–L559).
- Under "Local System" and a separator, generate 25 radio items for i = −12..+12.
- Caption "GMT" + sign + |i| ("GMT+0" for zero).
- Component name `N_View_TimeZone_GMT<i+24>`, which is also the persistence key.

**GM-093 Applying a time zone** (UnitFormMain: TFormMain.N_View_TimeZone_LocalClick L5418–L5436; DoTranslate L4689).
- "Local System" caption = translated text + " (" + sign + round(local offset in hours) + ")", e.g. "Local System (+3)".
- Shift = 0 for Local, else zone/24 − local offset (days).
- The shift is added to every displayed **absolute** time:
  - Details system time;
  - seek-bar clock row and bubble;
  - file list;
  - A/B labels.
- Data and map are unaffected.
- On a user click: refresh list strings and FormResize.

### 3.10 Details popup (TFormDetails)

**GM-120 When shown** (UnitFormMain: TFormMain.ImageSeekMouseMove L1935–L1961). Shown on mouse move over the seek bar or a graph image when all of these hold:
- Details is checked;
- not full screen;
- a file is loaded;
- border − 5 ≤ X ≤ W − border + 5.

Popup parameters:
- grWidth = (zoomR − zoomL) × 20 / max(32, W − 2 × border), the span of about 20 timeline pixels.
- Over a graph image and grWidth ≥ minZoom: that graph's class is used for a mini-graph. Over ImageSeek: no mini-graph.

The popup is hidden when the mouse leaves those images or that X range, on Details off, on graphics popup menu open, on file-list toggles, on video-area mouse move, and on close.

**GM-121 Form and profile** (TFORMDETAILS.dfm; UnitFormDetails: TFormDetails.FormCreate L112–L149, ApplyProfile L155–L170).
- Borderless, white, alpha 220/255, client area 296×201, 1-px black frame around the whole form, double-buffered.
- Profile rules:
  - controls named "*GSensor*" are visible only in GPS_G;
  - other "*GPS*" controls are visible unless Video;
  - in Video only the File, System Time and Video Time rows remain (height ≈ 45 px).
- Captions are right-aligned 2 px before their values. Values in the time block start at max(caption widths) + 4.

**GM-122 Content** (UnitFormDetails: TFormDetails.Popup L188–L293). P = "(?) " when the GPS fix is not good, else "".

| Row | Content |
|---|---|
| File | Sub-file name, prefixed "[k] " (1-based) when the group has more than 1 file. Shortened in the middle with "..." until it fits. |
| System Time | Locale date + long time of the position, plus the time-zone shift. |
| Video Time | "H:MM:SS.mmm" since the group start. |
| GPS | "[index]" |
| Longitude | P + "E"/"W" + \|lon\| with 6 decimals |
| Latitude | P + "N"/"S" + \|lat\| with 6 decimals |
| Altitude | P + round(m) + " m", or round(ft) + " feet"; just P if altitude = 0 |
| Bearing | P + round(deg) + "°" |
| Speed | P + km/h with ≤ 1 decimal (truncated, no ".0") + " km/h", or mph + " mph" |
| Track | P + round(m) with a space before the last 3 digits + " m", or miles with 3 decimals + " miles" |
| G-Sensor | "[index]" |
| x / y / z / \|V\| | " " or "-" + \|value\| with 3 decimals |

- When there is no GPS or G-sensor value at that instant, the corresponding block's values are blank.
- **Mini-graph** (only with a graph class):
  - Image 90 px high below the graph-name label (window height ≈ 204).
  - Cleared to colorGraphBG with a grey RGB(100,100,100) centre line.
  - The same DrawBackAuto rendering (GM-034–GM-038) over the window position ± grWidth/2, with width − 40 columns.
  - Cursor at the current play position. Label = graph name.

**GM-123 Placement and behaviour** (UnitFormDetails: Popup L281–L293, FormMouseMove L173–L176, Popdown L298–L303).
- Left = cursorX + 16.
- Top = max(cursorY + 24, screen bottom of PanelBottomGraphics).
- If there is no mini-graph, or the popup would cross the work-area bottom, place it above: top = panel top − popup height.
- Shown without taking focus.
- If the mouse moves onto the popup it jumps away: Left += X + 16, Top = min(Top + Y + 24, workBottom − Height).
- Each timer tick only the mini-graph cursor is redrawn (TimerTimer L1359). The text updates on the next mouse move.

### 3.11 Map

**GM-150 Provider selection** (UnitFormMain: TFormMain.N_View_GoogleMapsClick L2989–L2993; N_View_FileListClick L3002–L3046; NavigateResouse L5564–L5565). Provider codes:

| Code | Provider | Page |
|---|---|---|
| 1 | Google | gm.html |
| 2 | Yandex | ym.html |
| 3 | Yandex 2.0 | ym2.html |
| 4 | OSM / OpenLayers | osm.html |
| 5 | OSM2 / Leaflet (**default**) | osm2.html |
| 6 | Navitel | nm.html |
| 7 | Baidu | bm.html |
| 0 | None | `about:blank`, browser hidden |

- Pressing F3 (Google) while Google is already selected switches to None.
- Only when the provider changes: mark the map not initialised and load the page from the exe resources (`res://<exe>/<page>`).
- When None finishes loading while hidden, the browser height is set to 0 (WebBrowserDocumentComplete L3233–L3236).

**GM-151 JS execution guard** (UnitFormMain: TFormMain.ExecScript L1569–L1583). A script runs only when the map is initialised (or forced), visible and the document has finished loading. Otherwise the call returns "not run" (−1).

**GM-152 Callback protocol** (UnitFormMain: TFormMain.WebBrowserBeforeNavigate2 L3247–L3375).
- Navigations to `http://callback/<cmd>?<params>` are cancelled and processed only while the main form is visible.
- Params are `&`-separated `k=v` pairs, not URL-decoded except `%20` in `mt`.

| Cmd | When the page sends it | App action |
|---|---|---|
| `loaded` | Body onload | Forced `initialize(lat, lon, zoom, 'layer')` from registry `gm_Y`, `gm_X`, `gm_Z`, `gm_L` (defaults gmStartY / gmStartX / gmStartZoom / ""). Then run registry `mapPostAction` JS if non-empty. |
| `inited` | End of `initialize` | Map initialised = true; clear the route cache; RedrawRoute; redraw the arrow (no recentre). |
| `view` | Move end (also once after init) | As `bounds`, then redraw the arrow without recentring. Then hand keyboard focus back to the video window or file list (the map never keeps focus). |
| `bounds` | Every move | t/l/b/r = N/W/S/E edges; x/y = centre lon/lat; w/h = pixel size (fallback: browser size); z = zoom; mt = base-layer name. See the steps below. |
| `routeClick` | Click on a route polyline | Seek: for every sub-file of the current group find the nearest track point (squared degree distance). Seek to it only if it is within 10 map pixels of the click. The time is nudged ±0.5 s inside the segment at gap or end points. |

On `bounds` (and therefore `view`), the app:
1. stores the viewport;
2. computes metres per pixel (great-circle width and height across the centre / pixels) and metres per degree;
3. sets the arrow safe border = max(30 px, 15 % of view) per axis, in degrees;
4. sets the base arrow size = 20 px in metres;
5. persists `gm_X`/`gm_Y` (6 decimals) and `gm_Z` = clamp(zoom, 10, 17). Zoom is persisted only when not in the "whole route" overview, i.e. the restore-zoom value is unset;
6. persists `gm_L`;
7. on a zoom change, removes the position marker;
8. sets follow-mode = the current GPS position is inside the view;
9. runs RedrawRoute.

**GM-153 JS API the page must offer** (OSM2.HTML / OSM2_MAPS.JS reference; all pages share the names).

| Function | Effect |
|---|---|
| `initialize(lat, lon, zoom, layerName)` | Create the map with a scale control, a distance-measure control and a layer switcher. Select the base layer by name, falling back to "Open Steet Map" [sic]. Send `inited` and `view`. |
| `ClearPath()` | Remove all route polylines and S/F/P markers. |
| `AddPath([p…], "#RRGGBB", 0.4)` | Polyline, weight 6, opacity 0.4. Clicking it sends `routeClick`. |
| `AddPathMarker(img, p)` | Non-clickable 20×34 image marker anchored at its bottom centre. |
| `ClearMarkersAB()` | Remove the A/B pair. |
| `SetAB(imgA, pA, imgB, pB)` | Replace the A/B pair. |
| `PlaceMarker([p…])` | Position polygon: fill #00FF00 opaque, stroke #000000 2 px, on top. Replaces any circle. |
| `PlaceMarkerD(p, radius_m, 36)` | "Uncertain position" circle: fill #00FF00 at 50 % opacity, no stroke. Replaces the polygon. OSM2 quirk: an existing circle only moves its centre. |
| `ClearMarkers()` | Remove the position marker. |
| `PanTo(p)` | Animated pan. |
| `SetMapBounds(sw, ne)` | Fit bounds. |
| `SetMapZoom(z, anchor\|null, center)` | Zoom around the anchor, or a plain zoom when the anchor is null. |
| `AddKMLLayer(url)` / `AddGPXLayer(url)` | OSM2 only. Replaces the previous overlay and fits to it. |

- Point literals per provider (UnitUtilsMap: gmGetPositionStr L236–L248):
  - `[lat, lon]` for OSM2 and Yandex 2;
  - Web-Mercator integer metres for OpenLayers and Navitel (WGS84toGoogleBing L197–L217);
  - each library's own constructor otherwise.
- Coordinates use 6 decimals with "." (truncated).
- OSM2 extras:
  - base-layer list (OSM, OpenCycleMap family, MapQuest, 2GIS, Nokia/Here, Bing, Russian cadastre/topo layers, Esri, Stamen, Yahoo);
  - grey #cccccc background;
  - distance tool: a ruler button toggles click-to-add vertices on an editable polyline. Total shown as "N m" below 2000 m, else "N.N km"; vertex hover shows the cumulative distance; a cross button resets.

**GM-160 Route set-up** (UnitFormMain: TFormMain.SetRoute L1638–L1689). Called when a file or group is opened (ListViewSelectItem L2604–L2609, "panorama" = true, gap lines = true) and from the GPS-log converter.
1. Clone the GPS of each sub-file. Merge neighbours that share the same "selected" flag and are GPS-connected (end-to-start not a break, GM-175).
2. Panorama:
   - remember the current zoom capped at 14 as the restore-zoom (unless already set);
   - take the union bounding box, enlarged while its diagonal is < 400 m (x by max(0.0001°, w/3) per side, y by h/3 per side);
   - fit the map to it.
3. Before this, the arrow is reset and follow-mode = true. Afterwards a redraw runs.

**GM-161 Route drawing** (UnitFormMain: TFormMain.RedrawRoute L1728–L1873). Runs only with the map visible, initialised and a non-empty route.
1. **Window:** the visible bounds, snapped outward to a grid of the visible width dx (one extra screen each side), so small pans reuse the cache.
2. **Simplification tolerance:** the smallest power-of-two metres ≥ 4 px (minimum 1 m). Doubled up to 3 times while more than 1600 in-window points remain.
3. **Segments:** `GetTrack(tolerance)` (UnitMovingInfo: GetTrack L886–L902, LastRoutePoint L862–L885).
   - New segment at warned points and GPS breaks.
   - Keep the first and last point, and intermediate points at least `tolerance` from the last kept point.
4. **Visible runs:** for each segment, emit polylines only for runs whose points (or neighbours) are inside the window.
   - Colour **#FF0000**, or **#FF00FF** for "selected" infos (GPS-log converter selection). Opacity 0.4.
5. **Gap lines:** if gap lines are on, a **#444444** line joins a segment's last point to the next drawn point when farther apart than the tolerance.
6. **Time-point markers** (`gm_MarkerP.png`, a tiny red dot) at the first point of each new time bucket:

   | Setting | Bucket |
   |---|---|
   | 1 min | 1/1440 day |
   | 5 min | 5/1440 day |
   | 20 min | 20/1440 day |
   | Never | 100 days |

   Quirk: one dot appears at the start even with "Never".
7. **Start and finish markers:** `gm_MarkerS.png` on the first valid point of the whole route, `gm_MarkerF.png` on the last (salmon pins lettered S/F, 20×34).
8. **Cache:** the full script is skipped if it equals the last executed one. The cache is cleared on `inited`, ClearAll and time-points change.

**GM-162 Time Points menu** (UnitFormMain: TFormMain.N_View_TimePoints_1mClick L5035–L5040): persist, clear the route cache, redraw the route and the arrow.

**GM-165 Position marker** (UnitFormMain: TFormMain.TimerTimer L1291–L1315). Each tick (same activity conditions as GM-080):
- **No GPS at the instant:** `ClearMarkers()`.
- **Fix not good:** `PlaceMarkerD` with radius round(1.8 × base) (≈ 36 px), independent of the arrow size.
- **Otherwise:** polygon `PlaceMarker`. S = base arrow size (20 px in metres) × {Small ½, Normal 1/1.6, Big 1} (GM-166). Vertices are destination points from the position (bearing clockwise from north, distance in metres; UnitUtilsMap: gmGetCirclePoint L138–L185):

  | Case | Shape |
  |---|---|
  | Bearing unknown | Hexagon of radius S/2 at 30°, 90°, … 330° |
  | Small | Chevron: tip at bearing, distance S; points at bearing+120° and +240° at S/2; plus the centre |
  | Normal/Big | Speed-dependent arrow: half-angle a = max(20°, 60° − speed/3); tip length L = S × (1 + speed/300); points: tip, (bearing + 180 − a, S), centre, (bearing + 180 + a, S) |

**GM-166 Arrow size menu** (UnitFormMain: TFormMain.N_View_Arrow_SmallClick L3583–L3586): persist, then redraw immediately.

**GM-167 Follow and recentre** (UnitFormMain: TFormMain.TimerTimer L1319–L1334; SetMapCenter L1609–L1627; UnitUtilsMap: gmGetEllipseRadius L125–L132). Only on real timer ticks (not manual redraws):
1. **Restore-zoom on play:** if playing and a restore-zoom is set:
   - centre exactly on the car;
   - if the current zoom < restore-zoom, zoom to it around the car;
   - clear the restore-zoom; follow-mode = true.
2. **Follow:** if the car is outside the view shrunk by the safe border:
   - follow-mode on: recentre with look-ahead;
   - follow-mode off: nothing (follow resumes once a `bounds` callback sees the car inside the view).
3. **Look-ahead:** when the bearing is known, the new centre is 0.7 × r ahead of the car along its bearing. r is the radius of the ellipse whose half-axes are the distances from the centre to the safe border. Otherwise centre exactly.
4. The app pre-shifts its own viewport copy and calls `PanTo`.

**GM-168 A/B markers on the map** (UnitFormMain: TFormMain.DrawMarkersAB L1883–L1908). Called on every zoom-bar change.
- A = the position at **zoomL** (if > 0) and B = the position at **zoomR** (if < 1). These are the timeline *zoom window* edges, not the loop selection (Q9).
- A position equal to the route's start or finish point is dropped (original B comparison bug, Q9).
- A missing marker takes the other's position. If both are missing, `ClearMarkersAB()`; otherwise `SetAB` with `gm_MarkerA.png` / `gm_MarkerB.png` (a small black dot at the anchor).

**GM-170 Mouse wheel over the map** (UnitFormMain: TFormMain.FormMouseWheel L5373–L5390; GetMouseCoordinates L3209–L3216).
- Anchor A = the car position if follow-mode is on and a position is known; otherwise the geo-position under the mouse (from viewport and metres per pixel).
- New centre = A + (centre − A)/2^d.
- Call `SetMapZoom(zoom + d, A, newCentre)`, or `null` for A when A is unknown.

**GM-171 Post-load JavaScript** (UnitFormMain: TFormMain.N_View_Maps_PostActionClick L5630–L5659).
- Dialog 400×300, sizeable tool window, centred on the main form, caption = item caption.
- Memo in Lucida Console with both scrollbars, containing registry `mapPostAction`; OK button.
- On OK: save and reload the map page.
- In the Rust app this becomes "custom map init script/config" (Q10).

**GM-172 KML/GPX overlay** (UnitFormMain: TFormMain.N_View_Maps_OpenKMLClick L5845–L5871).
1. File dialog with filter "All supported formats (*.kml;*.gpx) | KML | GPX | All files".
2. Switch to OSM2 if it is not active.
3. Serve the file from a local HTTP server and call `AddGPXLayer` (".GPX" extension) or `AddKMLLayer`.
4. The overlay replaces the previous one and the map fits to it. Not persisted.

**GM-175 GPS break rule** (UnitUtilsMap: IsGPSBreak L472–L484). Two consecutive points are *connected* (not a break) if any of these holds, where dist = |Δlon| + |Δlat| in degrees:
- dist < 0.005 and Δt ≤ 11 s;
- dist ≤ 0.001 and Δt ≤ 15 min;
- dist ≤ 0.0002 and Δt ≤ 4 h;
- Δt ≤ 11 s and dist < 0.003°/s × Δt.

**GM-176 Distances** (UnitUtilsMap: gmGetDistance L94–L111, gmGetDistanceFast L114–L120).
- Distance = spherical law of cosines with R = 6 371 000 m. 0 when both deltas ≤ 8.993e-8°; cos clamped to [0, 1].
- Fast distance (only for spike filtering) = 111194.93 × (|Δlon| × (90 − |lat|)/90 + |Δlat|).

### 3.12 Coordinates menus

**GM-180 Flip and speed flags** (UnitFormMain: TFormMain.N_View_Coord_FlipXClick L5602–L5608). Shared by FlipX, FlipY and Coord_Speed.
- On a user click: toggle, persist, then stop and reload the current file so the corrections apply (GM-101, GM-104).
- Always: set the global "recalculate speed" flag = Coord_Speed checked (also at start-up).

**GM-181 Shift X/Y** (UnitFormMain: TFormMain.N_View_Coord_ShiftXClick L5613–L5623).
- Input box: caption "Fix Coordinates", prompt = item caption, value = registry `gm_shiftX` / `gm_shiftY` (shown default "0.000000").
- On OK: save and reload the file.
- The value is degrees; parsing accepts "," or ".".

**GM-182 Template variables** (UnitFormMain: R L5707–L5708, FillCoordinatesTepmalte L5710–L5740). Case-insensitive replace-all on the current instant's GPS value:

| Placeholder | Value |
|---|---|
| `{ye}` | 4-digit year |
| `{mo}` `{da}` `{ho}` `{mi}` `{se}` | 2-digit, zero-padded |
| `{ms}` | 3-digit milliseconds |
| `{date}` | Locale short date |
| `{time}` | Locale time |
| `{x}` / `{y}` | Lon / lat with 6 decimals |
| `{a}` | round(bearing) |
| `{s}` | round(km/h), or mph if British |
| `{p}` | round(track m), or miles with 3 decimals |
| `{h}` | round(altitude m), or feet |
| `{z}` | Current map zoom |
| literal `\r` or `\n` | CRLF |

Help text shown in the setup dialogs:
"{x}, {y}, {h} - coordinates / {z} - map zoom / {s} - speed / {a} - direction / {p} - track position / {date}, {ye}, {mo}, {da} - date / {time}, {ho}, {mi}, {se} - time".

**GM-183 Marker command setup** (UnitFormMain: TFormMain.N_View_MarkerURLSetupClick L5750–L5765).
- Input box titled "Enter command to run for marker" with an examples and variables help text:
  - `notepad.exe "{z}_{x}_{y}.txt"`
  - `http://www.google.com/maps/@{y},{x},{z}z`
  - "Don't open, only download:" `@http://localhost:8111/add_node?lon={x}&&lat={y}`
- Registry `markerURL`, default `http://www.openstreetmap.org/?mlat={y}&mlon={x}&zoom={z}`.

**GM-184 Execute marker command** (UnitFormMain: TFormMain.N_View_MarkerURLOpenClick L5772–L5803), key **M**.
1. Beep. If there is no GPS value at the current instant: beep again after 200 ms and stop.
2. Pause playback if playing.
3. Fill the template (GM-182), then:
   - `@http://…`: silent HTTP GET with a 5 s timeout, response ignored;
   - `http://…`: open in the default browser;
   - otherwise run as a command line. The program is the leading quoted string, or the text up to the first space; the rest (left-trimmed) is the arguments.

**GM-185 Copy setup** (UnitFormMain: coordCopyDef L5809–L5814, TFormMain.N_View_Coord_CopySetupClick L5820–L5828).
- Input box titled "Enter template for 'copy to clipboard'" with the variables plus " \n - new line".
- Registry `coordCopyTemplate`. Default depends on units:
  - metric: `{y}, {x}\n{date} {time}\nspeed: {s}km/h, direction: {a}°, track: {p}m`;
  - British: `… speed: {s}mph, direction: {a}°, track: {p}mile`.

**GM-186 Copy current point** (UnitFormMain: TFormMain.N_View_Coord_CopyClick L5831–L5839), key **C**: beep (double beep if there is no GPS value), fill the template and put it on the clipboard.

### 3.13 GPS Log Converter (TFormGPSLog)

**GM-190 Purpose and opening.** A converter for GPS log files, not a viewer.
- Opened by File ▸ "Convert GPS Log" (Ctrl+G) and the hidden list item "Show Related GPS Log" (UnitFormMain: TFormMain.N_File_Convert_GPSClick L3969–L3971, TFormMain.N_FileList_Track_GPSLogClick L3978–L3995; UnitFormGPSLog: TFormGPSLog.Popup L111–L142).
- Window "GPS Log Converter", DFM 575×627, placed at main form +30/+70, height ≤ main client height − 10.
- **ListViewFiles** (client area, checkboxes, multi-select). Columns:

  | Column | Width |
  |---|---|
  | "File" | 350 |
  | "Points" | 55 |
  | "Duration" | 70 |
  | "Track, km" (or miles) | 60 |

- **Bottom panel** (128 px):
  - converter check-list (left; output formats "name - .ext", first checked);
  - icon buttons Add / Remove / Clear / Delete-from-disk;
  - check boxes "Select all", "Show track on map" (default on) and "Merge into one file";
  - a 146×48 "Convert" button.
- Files can be dropped onto the window.

**GM-191 Map preview** (UnitFormGPSLog: ListViewFilesSelectItem L339–L351, TimerDrawRouteTimer L360–L382). When "Show track on map" is on, selecting or checking rows starts a timer (100 ms if < 2 rows involved, else 500 ms). The timer then:
1. loads every selected or checked file;
2. sends them to the main map via SetRoute (GM-160) with panorama on and gap lines off;
3. colours selected rows **#FF00FF** and checked-only rows **#FF0000**; a single file is red.

**GM-192 Convert** (UnitFormGPSLog: ButtonConvertClick L235–L280).
- With nothing checked, show "Select files to convert them".
- Otherwise write each checked file with each checked format, or merge all into one file named from the info name with ":" replaced by "-". Then open the folder in Explorer.

### 3.14 Handlers explicitly out of scope (covered by other specs)

- Playback, video and file list:
  - Play/Stop/frame buttons, `TrackBarVolumeChange`, `PanelPlaySpeed`/`CreateSpeedButtons`;
  - `N_FullScreenClick`, `ButtonVideoZoomOutClick` and the video-zoom menu, `ApplicationEventsShortCut`;
  - all `N_FileList_*`, `ListView*` and `N_Playback_*` handlers.
- Timeline actions:
  - `N_View_Time_SetAClick` and `N_View_Time_LoopABClick` (A/B selection semantics; only their bands are specified here, GM-063);
  - `N_View_Movie_OverlapClick`, `Seek`.
- Infrastructure:
  - `WebBrowserAds*`, `ExecAnalytics` (ads and analytics; **do not reimplement**);
  - `WebBrowserNavigateComplete2` (installs an IE UI handler to suppress the browser context menu; no equivalent needed);
  - `N_FileList_Track_GoogleClick` (exports KML to %TEMP% and opens it in Google Earth; file-list spec);
  - `TimerOnStartTimer` (renderer and start-up file loading).

---

## 4. Layout profiles

Common to all profiles: `PanelLeft` fills the remaining width with the video on top. `PanelBottom` (alBottom) holds the zoom bar (17 px) and the seek bar (40 px), then the graphs.

### 4.1 "My dash cam has GPS and G-Sensor" (N_View_Profile_GPS_G, default)

| Element | State and size |
|---|---|
| PanelRight | Visible; width `panel_Right` (510 default; 510 if it was < 20) |
| Map | alTop in PanelRight2, height round(0.6 × column), provider as persisted (Google after a user profile switch) |
| SplitterListView | Visible (7 px) if list and map are both visible |
| File list | alClient below the map; columns km and km/h visible (54/30 px restored) |
| PanelControl | Inside `PanelGraphicsAddon` (alRight in PanelBottom), alClient |
| PanelGraphicsAddon width | min(l + w, PanelBottom.Width/2) ≈ 255 px with default sizes (initially 260) |
| Gauges | Speed, G-sensor and Compass visible |
| Graphs (after profile click) | Speed + G-Sensor Value; V/X/Y/Z as persisted |
| PanelBottom height | `panel_Bottom`, ≥ 185 when graphs are on (320 when all seven are on) |
| SplitterBottom | Visible if any graph |
| Menus | Maps, Arrow, Time Points, Graphics, Fix Coordinates and Measurement enabled; G/V/X/Y/Z items visible |
| Details popup | GPS and G-sensor blocks |

### 4.2 "My dash cam has GPS" (N_View_Profile_GPS)

Same as 4.1 except:

| Element | State and size |
|---|---|
| Gauges | Speed and Compass; the compass sits under the speed gauge (121, 104) |
| Graphs (after profile click) | Speed on; G, V, X, Y, Z forced off |
| Menus | G/V/X/Y/Z graph items hidden; "Show All" turns on only S and H |
| Details popup | GPS block only |

### 4.3 "My dash cam can record only video" (N_View_Profile_Video)

| Element | State and size |
|---|---|
| Map | Provider forced to None (after a user click); browser hidden; list fills PanelRight2 |
| PanelControl | Back in PanelRight, alBottom, strip height 56 px (buttons, play speed, frame buttons, volume, loop) |
| PanelGraphicsAddon | Width 0 |
| Gauges | All hidden |
| Graphs | All hidden ("Hide All Graphics"); PanelBottom height = 57 (zoom bar + seek bar); SplitterBottom hidden |
| File list | Columns km and km/h set to 1 px width |
| Menus | Maps, Arrow, Time Points, Graphics, Fix Coordinates and Measurement disabled |
| Details popup | File / System Time / Video Time only |
| Timer | Gauges and map are not updated (GM-080, GM-165) |

### 4.4 Panel toggles (any profile)

| Toggle | Effect |
|---|---|
| File List off | List and splitter hidden; map fills the right column |
| Control Panel off | `PanelControl` hidden (in GPS profiles `PanelGraphicsAddon` keeps its last width, Q3) |
| List, map and control panel all off | PanelRight and SplitterRight width 0 (Q4) |
| Details off | Popup never shown |

---

## 5. Open questions

1. **Registry hive.** `regDefHKey` was not seen being initialised; HKCU is assumed. The key is `\Software\DATAKAM PLAYER\`. `\Software\Registrator Viewer\` is only read once for an `id` value at start-up.
2. **Profile → Google Maps.** A user profile switch to GPS/GPS_G selects *Google Maps*, not the default OSM2 (N_View_Profile_GPSClick L5483–L5489). Reproduce literally, or select the app's default map?
3. **Hidden control panel in GPS profiles.** `PanelGraphicsAddon` is not resized when Control Panel is toggled off, which probably leaves an empty area of its last width at the right of the graphs. Verify on the real app; the Rust app should probably collapse it.
4. **PanelRight not collapsing.** GM-016 counts `PanelControl.Visible` toward "right column has content" even when the control panel lives in the bottom panel (GPS profiles). Hiding list and map then leaves an empty right column. This is likely unintended.
5. **Stale gauges.** Speed and compass keep their last needle when the position has no GPS data. Only ClearAll resets them.
6. **Speed dial.** The dial artwork is fixed at 0–200 (km/h graduation) and reused unchanged for mph. The needle is unclamped above 200 (keeps rotating).
7. **Graph cursor colour.** NOT-MERGE raster op: black on white, and a different colour over traces. A plain 1-px black (or inverted) line is a reasonable port.
8. **Zoom scrollbar off-by-one.** Position is written as round(max × zoomL) but read as (pos − 1)/(max − 1) because DFM Min = 1. Reproduce or fix?
9. **Map A/B markers.** They show the timeline *zoom-window* edges, not the A-B loop selection. Confirm with the user which is wanted. The original also compares B against first.y instead of last.y when suppressing it at the finish point.
10. **Map providers.** Most embedded tile services and JS APIs (Google v3.18 key-less, Yandex 1.x, Navitel, Baidu 1.3, MapQuest, Nokia, Yahoo) are dead. Decide which to keep, and what replaces `mapPostAction` (user JS) and the distance tool in a native map.
11. **OSM2 uncertain-position circle.** Its radius is not updated when the circle already exists. This is an original quirk.
12. **Unused data.** GPS item +48 (longitudinal acceleration from GPS speed) is computed but no consumer was found. The last G-sensor sample's V and G are always 0.
13. **Marker command window.** Commands are started with SW_HIDE (hidden window). Use a normal show in the port?
14. **Template times.** Whether `{date}`/`{time}` include the user time-zone shift was not confirmed. The GPS item time is used directly, so most likely no shift.
15. **Separator bands.** Adjacent files whose ceil/floor columns overlap produce inverted rectangles; GDI normalises them. Check the visual result.
16. **Minimum graph height.** The decompilation shows max(free div count, 5) for non-last graphs, but the order of the `Max` and division was partly lost. Graphs never become smaller than 5 px.
17. **Details refresh.** The text is not refreshed on timer ticks while the mouse is still, only the mini-graph cursor. Keep this?
18. **KML before map init.** Loading a KML right after a provider switch is silently dropped because the map is not yet initialised.
19. **No-GPS time-point marker.** With "Don't show" one P dot is still emitted at the first point (hidden under the S pin).
