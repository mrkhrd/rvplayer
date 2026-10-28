# Playback, seeking and the video window (main window)

Source of truth for reimplementing the original "Registrator Viewer" 6.0.0.22 (Delphi 7) behaviour in rvplayer.
Everything here was recovered from the decompiled units in `rv_analysis/out/decomp` and the DFMs. Citations use the
form `Unit: Class.Method Lnnn–Lmmm`, where the L numbers are the original Pascal source lines. Unless another unit is
named, the unit is `UnitFormMain` and the class is `TFormMain`. Constants marked "(binary)" were read directly from the
unpacked executable because the decompiler did not show them.

---

## 1. Overview

* The player opens one **file item** at a time. A file item is either a single video or a **group** of consecutive
  videos (dashcam segments). Every position in the app is a single number, **progress `p` ∈ [0,1]**. It maps
  *linearly to wall-clock time* over the group's span, from the start of the first file to the end of the last.
  Each file (subitem) owns a sub-range `[pStart_i, pEnd_i]`, and the gaps between files are part of the span.
* One function, `GetCurrentPosition`, returns the current `p` and caches it as `lastPosition`. A periodic UI timer
  (`TimerTimer`) feeds that `p` to every consumer: the seek bar, the graphs' cursor, the map marker, the gauges
  and the details popup. When the user drags the seek-bar pin, `GetCurrentPosition` returns the *drag* value
  instead, so every consumer follows the drag. The video itself moves only when the pin is released.
* Play and pause are a single toggle (the Play button stays pressed while playing). Stop unloads the file
  completely. The UI timer runs only while playing. When paused, the UI is refreshed only by explicit calls after
  each user action.
* The timeline (seek bar and graphs) can be zoomed: a window `[zoomL, zoomR]` of `p`. While playing, the window
  scrolls automatically. An A–B selection `[selectL, selectR]` can be set and looped.
* The video picture supports these zoom modes: Letterbox (default), Pan & Scan, Stretch, Non-linear (EVR only), and
  Custom zoom with mouse panning. It also supports flips (VMR only), full screen, and frame or window snapshots to JPEG
  with EXIF data.
* The app draws **no overlay or subtitles on the video**. Subtitles (`UnitSubtitles`) are used only to generate SRT
  when a video is exported (see §5.3).

---

## 2. UI elements

Positions are design-time DFM values (`TFORMMAIN.dfm`). Many of them are re-laid-out at runtime by
`AlignControlPanel` / `FormResize`. The English text is from `TRANSLATE_EN.TXT`. When a bitmap loads successfully,
`LoadBitBtn` clears the button caption, so the buttons show glyphs only (L737–L739).

### 2.1 Control panel (`PanelControl`, alBottom of `PanelRight`, 510×132)

| Control | Type / parent | Geometry (DFM) | Caption / hint | Notes |
|---|---|---|---|---|
| `ButtonPlay` | TSpeedButton in `PanelControl_s1` (4,4,105×48) | 0,0, 48×48 | hint none; menu twin "Play / Pause" | `GroupIndex=1`, `AllowAllUp=True` → it is a **toggle** whose `Down` state means "playing". `Enabled=False` until a file is selected. Glyph `Btn_Play.bmp`, 96×24 = 4 frames of 24×24 (up = play triangle, disabled, clicked, down = **pause bars**). Flat (L913). |
| `ButtonStop` | TSpeedButton, `PanelControl_s1` | 57,0, 48×48 | menu twin "Stop" | `Enabled=False`. Glyph `Btn_Stop.bmp`, 2 frames of 24×24. Flat (L914). |
| `ButtonFramePrev` | TSpeedButton in `PanelControl_s2` (4,64,105×23) | 0,0, 23×23 | hint = "Back Frames (B)" (caption of `N_Playback_Frame_Prev` + shortcut text, L4706) | `Enabled=False`. Glyph `Btn_FramePrev.bmp`, 2×16×16. |
| `ButtonFrameNext` | TSpeedButton, `PanelControl_s2` | 25,0, 23×23 | hint = "Next Frame (N)" (L4707) | Glyph `Btn_FrameNext.bmp`. |
| `ButtonVideoZoomOut` | TSpeedButton, `PanelControl_s2` | 57,0, 23×23 | hint = caption of `N_VideoZoom_ZoomOut` "Zoom Out" (L4705) | Glyph `Btn_VideoZoomOut.bmp` (2×10×10). |
| `ButtonVideoZoomIn` | TSpeedButton, `PanelControl_s2` | 82,0, 23×23 | hint = "Zoom In " (L4704) | Glyph `Btn_VideoZoomIn.bmp` (2×16×16). |
| `PanelPlaySpeed` | TPanel, `PanelControl` | 4,90, 108×17 | hint key `PlaySpeed` "Play Speed" | `Enabled=False`. At runtime it holds 6 speed buttons (§3.8). |
| `TrackBarVolume` | TTrackBar, `PanelControl` | 0,108, 113×24 | hint key `Volume` "Volume" | Min 0, Max 100, Position 100, PageSize 10, Frequency 10, ThumbLength 15, TabStop False, horizontal. |
| `CheckBoxLoop` | TCheckBox, `PanelControl` | 118,6, 105×17 | caption key `N_View_Time_LoopAB` "Loop A-B"; hint `N_View_Time_LoopABHint` "Loop selected path A-B" | Synchronised with the `N_View_Time_LoopAB` menu item. |
| `ImageSpeed`, `ImageGSensor`, `ImageAzimut` | TImage | — | — | Gauges. They are redrawn from the position (§3.3). Their drawing belongs to the gauges spec. |

`AlignControlPanel` (L1371–L1478) flows these controls in columns and sets the `PanelControl` height to 60, 106 or
135 px, depending on which gauges are visible. That layout belongs to the layout spec.

### 2.2 Timeline (`PanelBottom`, alBottom of `PanelLeft`, height 185, persisted as `panel_Bottom`)

| Control | Type / parent | Geometry | Notes |
|---|---|---|---|
| `PanelBottom` | TPanel | alBottom, h 185 | Has `OnMouseDown/Move/Up` = ImageSeek handlers. In full screen it is re-parented into the full-screen form (§3.12). |
| `PanelGraphicsAndZoom` | TPanel | alClient | Contains the zoom strip and the graphs. |
| `PanelZoomScroll` | TPanel | alTop, h 17 | Holds the following three controls. |
| `ButtonZoomOut` | TSpeedButton | 0,0, 17×17, caption `-`, Flat | Glyph `Btn_ZoomOut.bmp` 12×12. Time zoom out. |
| `ScrollBarZoom` | TScrollBar | 17,0, w = `ButtonZoomIn.Left − ButtonZoomOut.Right` (L1524), h 17 | Min 1, Max 4, Position 2, LargeChange 10, TabStop False. Pans the zoomed time window. |
| `ButtonZoomIn` | TSpeedButton | Left = client width − 17 (L1523), 17×17, caption `+`, Flat | Glyph `Btn_ZoomIn.bmp`. Time zoom in. |
| `PanelBottomGraphics` | TPanel | alClient | Has the ImageSeek mouse handlers and `PopupMenuGraphics`. The graph images below the seek bar share the same mouse handlers (L2915–L2917). |
| `ImageSeek` | TImage (replaced at runtime by a subclass with `OnMouseLeave` → `OnImageMouseLeave`, `ReplaceSeekImage` L687–L700) | alTop, h 40 | **The seek bar.** Popup `PopupMenuGraphics`. Font size 8. `border` = ⌊TextWidth("99:99:99")/2⌋ + 4 px. Pin diameter `pinH` = 10 px (UnitBitmaps: TSeekBitmap.Create L401–L408). |
| `PanelGraphicsAddon` | TPanel | alRight, w 69 | The layout spec covers this panel. |

### 2.3 Video area

| Control | Type / parent | Notes |
|---|---|---|
| `PanelVideo` | TPanel in `PanelLeft`, alClient, `BorderStyle=bsSingle`, `BevelOuter=bvNone` | DFM colour clGray, set to **black** at runtime (L947). Popup `PopupMenuVideoWindow`. `OnDblClick=N_FullScreenClick`, `OnMouseDown/Move/Up` = `VideoWindowWMR*`. Re-parented into `FormFullScreen` in full screen, with `BorderStyle=bsNone`. |
| `VideoWindowWMR` / `VideoWindowEVR` | TVideoWindow (VMR) or TPanel hosting EVR | Recreated whenever the renderer changes (L2814–L2844). They copy the double-click, mouse handlers and popup from `PanelVideo`. **They are positioned to the visible part of the picture** (`lastVideoDisplayRect`), not to the whole panel (§3.10). |
| `FormFullScreen` | separate TForm, `BorderStyle=bsNone`, caption "Full Screen" | Hosts `PanelVideo` (and, on demand, `PanelBottom`) in full screen (UnitFormFullScreen). |

### 2.4 Timers

| Timer | Interval | Purpose |
|---|---|---|
| `Timer` (`TimerTimer`) | DFM 100 ms, `Enabled=False`. Set by the "Panels Refresh" menu: Slow **1000**, Normal **300**, Fast **100** ms, or **200** ms when Fast is checked and the map provider `N_View_YandexMaps2` is selected (`N_View_Refresh_SlowClick` L3568–L3580). | Main UI loop. Enabled only while playing (§3.2). |
| `TimerMouseHide` | 2000 ms, `Enabled=False` | Hides the mouse cursor over the video while playing (§3.11). |
| `TimerOnStart` | 1 ms, enabled | One-shot start-up: checks the renderer, opens command-line files and auto-plays them (§3.15). |

### 2.5 Menu items in scope (main menu; caption key = item name)

Shortcut values are decoded from Delphi TShortCut. Each menu bitmap is loaded from `<ItemName>.bmp`
(`LoadMenuBitmaps`). For example, `N_Playback_Play.bmp`, `N_Playback_Left.bmp`, `N_View_Time_SetA.bmp` and
`N_VideoZoom_ZoomIn.bmp` exist in RCDATA.

**Playback** (`N_Playback`, "Playback")

| Item | English | Shortcut | Kind | Handler |
|---|---|---|---|---|
| `N_Playback_Play` | Play / Pause | **Space** | command | `N_Playback_PlayClick` |
| `N_Playback_Stop` | Stop | **Esc** | command | `N_Playback_StopClick` |
| `N_FullScreen` | Full Screen | **Alt+Enter** | check | `N_FullScreenClick` |
| `N_Playback_Left` | Jump Back | **Left arrow** | command | `N_Playback_LeftClick` |
| `N_Playback_Right` | Jump Forward | **Right arrow** | command | `N_Playback_LeftClick` |
| `N_Playback_Frame_Prev` | Back Frames | **B** | command | `ButtonFramePrevClick` |
| `N_Playback_Frame_Next` | Next Frame | **N** | command | `ButtonFrameNextClick` |
| `N_Playback_PauseOnClick` | Pause on Click | — | check (default off) | `N_Playback_PauseOnClickClick` |
| `N_Playback_PlayNext` | Auto Switch to Next File | — | check (default off) | `N_Playback_PlayNextClick` |
| `N_Playback_Save_Frame` | Save Current Video Frame | **Alt+S** | command | `N_Playback_Save_FrameClick` |
| `N_Playback_Save_Wind` | Save Current Window | **Alt+W** | command | `N_Playback_Save_FrameClick` |
| `N_Playback_Splitters` → `_Int_MOV` (checked), `_Int_AVI`, `_DSShow` (checked) | "Use internal %format% splitter" / "Use preinstalled (from DirectShow)" | — | checks | `N_Playback_Decoder_MSVideo_ForceClick` |
| `N_Playback_Decoders` → MSVideo Force/Auto(default)/Never, MSAudio Force/Auto(default)/Never | "Using Microsoft DTV-DVD Video/Audio Decoder" + Force/Auto/Never | — | radio groups 1 and 2 | same |
| `N_Playback_Renderer` → `_NUL` "Simple", `_WMR` "VMR (Windows XP)", `_EVR` (default) "EVR (Windows Vista / 7, hardware acceleration DXVA)" | | — | radio | `N_Playback_Renderer_NULClick` |

**Zoom** (`N_VideoZoom`, "Zoom"; every item is in GroupIndex 1)

| Item | English | Shortcut | Kind |
|---|---|---|---|
| `N_VideoZoom_Custom` | Custom (its caption gets " (NN%)" while it is checked) | — | radio |
| `N_VideoZoom_ZoomIn` / `_ZoomOut` | "Zoom In " / "Zoom Out" | (Num+ / Num−, see §3.14) | command |
| `N_VideoZoom_800/400/200/100/50/25/10` | captions "800%" … "10%". `100%` gets `"                      *"` appended (L964), which displays a `*` hint for the Num* shortcut. | — | command |
| `N_VideoZoom_Other` | Other | — | command (input dialog) |
| `N_VideoZoom_Letterbox` | "(Letterbox) Fill in the window, keeping the aspect ratio" | **Alt+1** | radio, default checked |
| `N_VideoZoom_PanAndScan` | "(Pan and Scan) Stretch to entire window, keeping the aspect ratio" | **Alt+2** | radio |
| `N_VideoZoom_Stretch` | "Stretch to entire window, without keeping the aspect ratio" | **Alt+3** | radio |
| `N_VideoZoom_NotLinear` | "Not linear stretching, keeping the aspect ratio in the center" (enabled only with EVR. When disabled, " (EVRonly)" is appended) | **Alt+4** | radio |
| `N_VideoZoom_FlipV` / `_FlipH` | Flip Vertical / Flip Horizontal (enabled only with VMR, otherwise " (VMRonly)" postfix) | **F9** / **F10** | check |

**View → Timeline** (`N_View_Time`, "Timeline")

| Item | English | Shortcut |
|---|---|---|
| `N_View_Time_ZoomIn` | Zoom In | **Ctrl+=** (VK_OEM_PLUS) |
| `N_View_Time_ZoomOut` | Zoom Out | **Ctrl+−** (VK_OEM_MINUS) |
| `N_View_Movie_Overlap` | Set video files overlap time | **Ctrl+J** |
| `N_View_Time_SetA` | Set selection start (A) in current position | **Ctrl+Home** |
| `N_View_Time_SetB` | Set selection end (B) in current position | **Ctrl+End** |
| `N_View_Time_LoopAB` | Loop A-B | **Ctrl+L** (check) |

Other View items that touch this area are `N_View_Refresh_Slow/Norm/Fast` ("Panels Refresh": Slow, Normal,
Fast = default) and `N_View_TimePoints_1m/5m/20m/Never` ("Time Points": 1 minute, 5 minutes, 20 minutes,
Don't show = default). `N_View_Details` (F8) enables the hover popup.

### 2.6 Context menus (built at runtime by `CopyMainMenu` L631–L653)

* **`PopupMenuVideoWindow`** (right-click on the video) contains:
  1. the `N_Playback` items, except the Splitters, Decoders and Renderer submenus;
  2. a separator, then the top-level File, View and Zoom menus as submenus (Playback and Help are left out);
  3. a separator, then Exit.
* **`PopupMenuGraphics`** (right-click on the seek bar or graphs) contains:
  1. the `N_View_Time` items except LoopAB, then `N_File_Save_Video`;
  2. a separator, then the `N_View_Graphics` items;
  3. a separator, then `N_View_Details`.

  Opening it hides the details popup (`PopupMenuGraphicsPopup` L4995–L4996).
* The copied items share the original's caption, checked/enabled/visible state, `OnClick` and shortcut, and stay
  synchronised through `MenuItemCopy` (L2736–L2767). A handler that receives a copy resolves it to the original
  with `GetRelatedMenuItem` (L2786–L2795).

---

## 3. Requirements

### 3.1 Position model (single source of truth)

**PV-001 Progress definition.** The global position is `p` ∈ [0,1] over the current file item (group).
`p` corresponds to wall-clock time `T(p) = groupEnd + (p − 1)·groupDuration`. Times are TDateTime (days), and
`groupDuration` is the group's total wall-clock span, gaps included. When there is no current subitem
(`currentSubitemIndex < 0`), the time is 0. — `ProgressToSystemTime L2241–L2246`

**PV-002 Subitem ranges.** Each file (subitem) `i` of the group has a global range `[pStart_i, pEnd_i]`.
* Local progress is `l = (p − pStart_i)/(pEnd_i − pStart_i)`. It is valid only when `0 ≤ l < 1`, and is `0` when
  the range is empty.
* The inverse is `p = pStart_i + l·(pEnd_i − pStart_i)`.

Media time inside the file is `l × fileDuration`. — `GetLocalProgressVideo L2180–L2189`, `GetGlobalProgressFromVideo L2168–L2171`

**PV-003 Which file owns a position.** `ProgressToSubitemIndex(p)` checks the files in order and returns the first
`i` for which `p < pEnd_i` or `i` is the last file, provided that either `pStart_i ≤ p`, `i = 0`, or
`pEnd_{i−1} ≤ p`. In other words, **a position inside a gap belongs to the following file**. If no file matches,
it returns 0. — `ProgressToSubitemIndex L2220–L2230`

**PV-004 Video time / file label.**
* `ProgressToVideoTime(p) = round(p · groupDuration · 86 400 000)` ms. This is elapsed time from the group start,
  and it is 0 when there is no subitem.
* `ProgressToFile(p)` returns the file name of the owning subitem. If the group has more than one file, a suffix
  with the 1-based index is added.

— `ProgressToVideoTime L2233–L2238`, `ProgressToFile L2251–L2256`

**PV-005 GetCurrentPosition.** The function returns `p` and always stores it in `lastPosition` (L2008). Rules, in order:
1. While the seek pin is being dragged (`pinGet`), return `pinGetValue`. The video is not queried.
2. If the player is not Paused or Running, has no seeking interface, or the media position query fails, return
   `lastSeekPos` (the last value set by Seek).
3. With the internal source filter (the whole group is one stream), convert the stream time (ms) to
   `(index, local)` with `TSourceGroup.TimeToProgress`, then map it to global with that subitem's range (L1989–L1991).
4. Otherwise (one DirectShow graph per file): read `cur` and `dur` of the current file.
   * **Auto-advance inside a group (L1994–L1996).** If a next subitem exists and `cur ≥ dur − 1` (in 100 ns units,
     that is, the end of the file has been reached), call `Seek(max(pStart_{i+1}, pEnd_i))` without the UI refresh
     and return the new `lastSeekPos`. Playback continues with the next file of the group. Any time gap between the
     files is skipped.
   * Otherwise `local = cur/dur` (0 when `dur ≤ 0`), mapped to global (L1997–L2002).
   * **Rate compensation (L2003–L2004).** If `p > lastSeekPos` and the graph rate ≠ 1, then
     `p = lastSeekPos + (p − lastSeekPos)/rate`. See open question Q1.

— `GetCurrentPosition L1984–L2009`

**PV-006 Seek(p, callTimer, forceVideoSeek).** — `Seek L2017–L2104`
1. Store `lastSeekPos = p`, clear `pinGet` (this ends any drag) and set `gmArrowInCenter = true` (the map recentres
   on the next tick). Remember the player state `st` (L2019–L2022). This step happens **before** validation.
2. Ignore the call (return) if no subitem is loaded, if `p < 0`, or if `p > 1` (L2024).
3. Find the owning subitem `k` (PV-003). If `p < pStart_k` (the position is in a gap), clamp `p = pStart_k`
   (L2026–L2029).
4. If `k ≠ currentSubitemIndex`, set `currentSubitemIndex = k`. When playing per-file (no internal group stream):
   * If `forceVideoSeek` is set and `st` is neither Paused nor Running, treat `st` as Paused.
   * If `st` is Paused or Running: stop the player, open file `k` (`RenderFile`), store its real duration if it was
     unknown (then refresh the layout), and re-lay-out the video for the new frame size (`ButtonVideoZoomOutClick(nil)`).
   * If `st` is Stopped and not forced, the index changes but nothing is loaded.

   (L2032–L2058)
5. Convert to the file's local progress, then to a media time: `round(local·dur)`, or `TSourceGroup.ProgressToTime`
   for the internal stream. Set the absolute position with no stop position (L2066–L2075). Reset the
   "frame rendered" counter (`IsRendered`, L2076).
6. **Read back** the position that was actually reached and set `lastSeekPos` to it (`cur/dur` mapped to global, or
   the group mapping) (L2079–L2087). The position therefore snaps to what the decoder can reach.
7. Restore the player state `st` if it changed (L2090–L2091). If step mode was active (after a frame step) and `st`
   is Paused, do Play then Pause, and clear step mode (L2093–L2098).
8. If `callTimer` is set, run one UI tick immediately (`TimerTimer(Self)`, L2101–L2102).

**PV-007 Frame-ready wait.** `WaitForFrame(timeout = −1 → 1000 ms)`. — `WaitForFrame L2109–L2138`, `WaitForRendered L2143–L2151`, `IsRendered L2156–L2165`
* It is re-entrancy guarded by `bWaitForFrameMode`.
* It first repaints the graph images and the seek bar.
* If the new frame is not rendered yet:
  * **Paused:** halve the volume, Play, wait until a frame is rendered or the timeout expires (polling messages
    every 1 ms), Pause, restore the volume.
  * **Running:** just wait.
* Finally it processes pending messages.
* "Rendered" means either the internal source filter's `rendered` flag is set, or the renderer's frames-drawn
  counter has increased since the last check.

### 3.2 Play / pause / stop

**PV-010 File selection loads, but does not open.** Selecting exactly one row in the file list (with a different
file than the one already loaded, or while Play is disabled) does the following:
1. Stops playback.
2. Loads the item's metadata and GPS/G-sensor data, and sets `currentSubitemIndex = 0`.
3. Enables `ButtonPlay` and `PanelPlaySpeed`.
4. Recomputes the zoom bar and layout.
5. Draws the route and runs one UI tick (L2571–L2609).

Re-selecting the item that is already playing does nothing (`IsNowPlaying` L2421–L2422, L2570). Multi-selection
posts a message that stops playback when the selection count ≠ 1 (L2563–L2564, `OnWmUser3` L2631–L2637).
— `ListViewSelectItem L2562–L2626`

**PV-011 Single click opens paused.** A plain click on the list opens the player paused at `lastPosition`, which
shows the first frame, when all of the following hold:
* `ButtonPlay` is enabled;
* the player is not yet active;
* neither Ctrl nor Shift is held.

It does this by releasing `ButtonPlay` and calling `ButtonPlayClick`. — `ListViewClick L2640–L2648`

**PV-012 Double click / Enter plays.** If the file is not loaded yet, Stop and load it (PV-010). Then press
`ButtonPlay` (Down) and call `ButtonPlayClick`. `N_FileList_Open` (Enter) uses the same handler.
— `ListViewDblClick L2657–L2665`

**PV-013 ButtonPlayClick.** — `ButtonPlayClick L1076–L1151`
1. If the player is not active:
   * If no file is loaded, do nothing.
   * Otherwise set the window caption (PV-014), show the app-start cursor, attach the renderer, and activate the
     player.
   * Force a file (re)load by setting `currentSubitemIndex = 9999` and calling
     `Seek(lastPosition, callTimer=false, force=true)`. The file opens **paused** at the last position.
   * Apply the volume (`TrackBarVolumeChange`).

   (L1078–L1109)
2. If `ButtonPlay.Down` (play requested): call Play unless already Running, enable `Timer`, and disable the screen
   saver. Otherwise: if Running, Pause; disable `Timer`; re-enable the screen saver (L1112–L1121).
3. Enable `ButtonStop`, `ButtonVideoZoomIn/Out` and `PanelPlaySpeed`. Enable `ButtonFramePrev/Next` and the
   `N_Playback_Frame_*` menu items if the player is active. Re-layout (`FormResize`). Restore the cursor
   (L1124–L1151, part of it from the binary).
4. If not in full screen, move keyboard focus to the video window and then to the file list, so that shortcuts keep
   working (from the binary, end of the method).

**PV-014 Window caption while a file is open.** The caption is built as:
`<file face>    <date-time>[    <track length>][    <avg speed><km/h | mph>]  -  DATAKAM PLAYER - Registrator Viewer[  -  <registrator model>]`.
After Stop it is reset to `DATAKAM PLAYER - Registrator Viewer[  -  <model>]`. The application title follows the
caption. — `ButtonPlayClick L1082–L1093`, `ButtonStopClick L1233–L1236`

**PV-015 Play/Pause command (Space).** If `ButtonPlay` is enabled, toggle its `Down` state and call
`ButtonPlayClick`. If no file is selected, nothing happens. — `N_Playback_PlayClick L3099–L3104`

**PV-016 Stop command (Esc).** In full screen, or when the full-screen form is the active form, Esc **leaves full
screen** instead of stopping. Otherwise it behaves like the Stop button. — `N_Playback_StopClick L3107–L3112`

**PV-017 ButtonStopClick.** — `ButtonStopClick L1198–L1238`, `ClearAll L1156–L1194`

Steps:
1. Disable `Timer`. Release and disable `ButtonPlay`. Disable Stop, VideoZoomIn/Out, FramePrev/Next and
   `PanelPlaySpeed`.
2. If the sender is the Stop button (or the Esc command), clear the file-list selection.
3. If the player is active: stop it, release the renderer and unload it completely.
4. Reset the caption.
5. `ClearAll`:
   * current item empty, `currentSubitemIndex = −1`;
   * screen saver re-enabled;
   * **leave full screen** if active;
   * uncheck Loop A-B;
   * clear the map path and markers;
   * reset the gauges and graphs;
   * `lastPosition = lastSeekPos = 0`, `bStepMode = false`;
   * **time zoom reset** to `[0,1]`;
   * **A/B selection reset** to `[0,0]`;
   * **video zoom reset** to factor 1.0, pan centre (0.5, 0.5);
   * if Custom zoom is active, switch back to Letterbox.

**PV-018 End of file, single file or last file of a group.** There is no end-of-stream handler. When the position
reaches 1.0, playback simply sits on the last frame. `ButtonPlay` stays Down and the timer keeps running. There is
no auto-rewind and no auto-stop. Toggling Play pauses; toggling again does nothing visible until the user seeks.
(The DFM `FilterGraph` has no completion event. `TimerTimer` is the only end-of-file logic.)

**PV-019 Auto switch to next file (`N_Playback_PlayNext`).** On a timer tick, if all of the following hold:
* the player is Running;
* the loop does not apply;
* `PlayNext` is checked;
* `p == 1.0` exactly;
* `ButtonPlay.Down`;

then:
1. Remember the list index `i` and clear the selection.
2. Stop playback.
3. Choose `i+1`, or **0 (wrap to the first row)** if `i` was the last row, and select it.
4. Scroll it into view.
5. Double-click semantics (load and play, PV-012).

The tick ends there. — `TimerTimer L1263–L1275`

**PV-020 Pause on click (`N_Playback_PauseOnClick`).** When this is checked, a **left** mouse-down on the video
toggles play/pause (PV-015). It applies only when the player is active and **Custom zoom is not active** (in Custom
zoom a left press starts panning instead). Because a double-click delivers a second mouse-down, a double-click
toggles twice. For the same reason, double-clicking the video does **not** toggle full screen while this option is on
(PV-070). — `VideoWindowWMRMouseDown L4374–L4383`, `N_FullScreenClick L2872`

### 3.3 The UI tick (`TimerTimer`) — `TimerTimer L1253–L1362`

The tick runs every `Timer.Interval` while playing. It is also called synchronously:
* with a sender by `Seek(callTimer)`, during a pin drag, by frame next and by full screen;
* without a sender by `FormResize`, list selection and `N_View_TimePoints`.

Each run does the following, in order:

**PV-021** The tick exits immediately if the main form is not visible (L1254). It reads
`p = GetCurrentPosition` (L1256), and exits if no subitem is loaded or `p < 0` (L1257).

**PV-022 Loop A–B (L1258–L1260).** This step runs only when the tick has a sender, the player is active and the
state is Running. If `CheckBoxLoop` is checked, `selectR > selectL` and `p ≥ selectR`, then `Seek(selectL)` (no UI
refresh) and skip PV-019 and PV-023. A position before A is **not** forced to A; playback runs normally until it
reaches B.

**PV-023 Timeline auto-scroll (L1262–L1286).** In the same "running tick" context, with `zoom = zoomR − zoomL`:
* If `p > zoomR − 0.05·zoom` and `zoomR < 1`: shift the window so that `zoomL = p − 0.1·zoom` (page-flip forward,
  cursor at 10 % from the left).
* Else if `p < zoomL`: centre the window on `p` (`zoomL = p − 0.5·zoom`).

After either shift, normalise the window (PV-042) and redraw the timeline background (`FormResize`).
The constants 0.05 and 0.1 are from the binary.

**PV-024 PlayNext check.** See PV-019 (evaluated before PV-023).

**PV-025 Map and gauges (L1291–L1351).** This runs when the main form is the active form, or when the full-screen
form currently hosts `PanelBottom`. It is skipped entirely in the "Video" profile (`N_View_Profile_Video`).
* Look up the GPS/G-sensor sample at `p` (`GetMovingInfo` L2206–L2215, which uses `ProgressToSystemTime`).
* If there is no sample, clear the map marker. Otherwise place the map arrow (Big = full size, Normal = size/1.6,
  Small = size/2), or a "dead" marker when the GPS fix is bad.
* On timer ticks, recentre the map when the arrow leaves the safe border.
* Redraw the speed/altitude gauge (metric, or imperial: speed /1.609344, track /1000/1.609344, altitude /0.3048),
  the azimuth gauge and the G-sensor gauge when the control panel is visible.

The details belong to the map and gauges specs. The point here is that they are driven by **the same `p`**.

**PV-026 Seek bar, graphs and details (L1354–L1359).** This step is skipped when neither the main form is active nor
the full-screen form hosts `PanelVideo` (for example, while a dialog is active).
1. Compute seconds per pixel: `spp = (zoomR − zoomL)·groupDuration / max(1, seekWidth − 2·border)`.
2. Pick the fractional digits shown on the pin's elapsed-time label: **3** (ms) if `spp < 0.01 s`, **1** (tenths)
   if `spp < 0.1 s`, otherwise **0**.
3. Draw the seek bar at `p`, draw each graph's cursor at `p`, and update the details popup at `p`.

The constants 1.157e-7 d = 0.01 s and 1.157e-6 d = 0.1 s are from the binary.

### 3.4 Seek bar (`ImageSeek`) geometry and drawing

**PV-030 X ↔ p mapping.** With `w = max(1, ImageSeek.Width − 2·border)` and the visible window `[minP,maxP] = [zoomL,zoomR]`:
* `f = clamp((x − border)/w, 0, 1)`;
* `p = minP + f·(maxP − minP)`.

The pin is drawn at `x = border + round(((p − minP)/(maxP − minP))·w)`, and only when that x lies within
`[border, Width − border]`. — UnitBitmaps: `TSeekBitmap.GetProgress L551–L556`, `TSeekBitmap.Draw L504–L511`

**PV-031 Pin hit-test.** The pointer is "on the pin" when `|x − lastPinX| < pinH/2` (5 px).
— UnitBitmaps: `TSeekBitmap.GetProgress L555`

**PV-032 Background (redrawn on every layout change, `FormResize` L1494–L1498).**
`TSeekBitmap.Prepare(minTime = groupStart + tzShift, maxTime = groupEnd + tzShift, minP = zoomL, maxP = zoomR)`.
UnitBitmaps: `TSeekBitmap.Prepare L417–L489`, `TMyBitmap.DrawDelections L119–L124`, `D L100–L118`.

Bands are computed per pixel by `CalculateCurrentSelections` (L2436–L2477) over width `w`, offset by `border`,
and filled over the full image height:
* **Gaps between files** (no video) are filled RGB(200,200,200). Where two files touch, a 1-px separator line is
  drawn, RGB(100,100,100) when there are fewer than 5 separator entries, otherwise RGB(200,200,200).
* **Files whose duration could not be determined** (`durationState = 2`) are filled RGB(240,240,0).
* **Locked files** are filled RGB(255,255,180).
* **The A–B selection** `[floor(w·(selectL−minP)/(maxP−minP)), ceil(...selectR...)]` is filled with the system
  highlight colour. It is drawn only if `selectL < selectR` and it intersects the window.

Each band is `[floor(x1), ceil(x2))`, clipped to `[0, w]`. Zero-width bands are drawn as a 1-px line.

**Upper ruler (elapsed time since group start).**
* "Seconds mode" applies when the visible span `t2 − t1 < 4 min` (0.0027778 d, from the binary).
* The step starts at 1 s (seconds mode) or 60 s, and is increased by the same base step until the number of labels
  fits `round(w / (TextWidth("99")+20))` (or `"99:99"` in seconds mode).
* Labels use `TimeToStrSmall`: `[N d ]h:mm` or `m`, with `:ss` appended in seconds mode
  (UnitUtilsStr: `TimeToStrSmall L329–L342`).
* Each label sits above the track, with a tick from the track centre up to `centre − pinH/2 − 2`. A label is
  skipped when it would overlap the previous one.

**Lower ruler (absolute clock time, `hh:mm:ss`).** Same idea, using `"99:99:99"` for the width budget. Ticks are
aligned to whole steps of time of day, and the labels sit below the track.

**Track.** A rounded rectangle from `x = border − pinH/2` to `Width − border + pinH/2`, vertically
`centre ± pinH/2`, corner radius `pinH − 1`, black pen, system button-shadow fill.

**PV-033 Foreground (every tick).** Copy the background, then at the pin x draw:
* a black vertical line from `centre + pinH/2 + 5` to `centre − pinH/2 − 5`;
* above, in a white rounded box with a grey(100) border, the **elapsed time** at `p` (`TimeToStrSmall` with
  seconds), plus `.d`, `.dd` or `.ddd` according to PV-026;
* below, in a white rounded box, the **clock time** `hh:mm:ss` at `p`;
* a pin circle of diameter `pinH`, black outline, filled with the system 3D-highlight colour.

Store `lastPinX`. — UnitBitmaps: `TSeekBitmap.Draw L500–L545`

### 3.5 Seek bar / graphs mouse interaction (`ImageSeekMouseDown/Move/Up`)

These handlers are attached to `ImageSeek`, every graph image, `PanelBottomGraphics` and `PanelBottom`.

**PV-034 Press.** Any button except the right one (right-click opens `PopupMenuGraphics`) is handled when no drag is
in progress. Compute `p = GetProgress(X)`.
* If the press is **on the pin of the seek bar itself** (`Sender = ImageSeek` and PV-031): start a drag. Set
  `pinGet = true`, `pinGetValue = p`. **Do not seek.**
* Otherwise (anywhere else on the seek bar, or anywhere on a graph): **seek immediately**,
  `Seek(p, callTimer=true, force=false)`.

A press-and-move that does not start on the pin does **not** scrub. — `ImageSeekMouseDown L1915–L1927`

**PV-035 Move.**
* **While dragging:** `pinGetValue = GetProgress(X)` (clamped to the visible window) and run a UI tick. Because
  `GetCurrentPosition` returns `pinGetValue`, the pin, labels, graph cursors, map marker and gauges all follow the
  mouse **without moving the video**.
* **Otherwise:** the cursor is a hand (`crHandPoint`) over the pin and the default cursor elsewhere.

— `ImageSeekMouseMove L1935–L1945`

**PV-036 Release.** If a drag is active, `Seek(GetProgress(X), callTimer=true)`. The video jumps only now. Seek
clears `pinGet`. — `ImageSeekMouseUp L1967–L1972`

**PV-037 Hover details popup.** If all of the following hold:
* `N_View_Details` is checked;
* the app is not in full screen;
* the pointer is over an image (seek bar or graph);
* a file is loaded;
* `X ∈ [border − 5, Width − border + 5]`;

then show the details popup for position `p`. The popup gets a half-width of
`grWidth = (zoomR − zoomL)·20 / max(32, Width − 2·border)` (≈ 20 px worth of time), the graph class under the
pointer (only when `grWidth ≥ minimum zoom`, PV-041), and `lastPosition`. Otherwise hide it.

The popup is also hidden on mouse leave, on any video mouse move, when the graphics popup menu opens and on close.
The popup's content belongs to the details spec. — `ImageSeekMouseMove L1946–L1958`, `OnImageMouseLeave L1975–L1978`

### 3.6 Time zoom

**PV-040 State.** `zoomL`, `zoomR` ∈ [0,1], with the default window `[0,1]`. The factor `zoomK` is **2.0**
(FormCreate L958). It is temporarily set to **1.5** for timeline wheel zoom and **1.2** for video wheel zoom (both PV-075).
The window is reset to `[0,1]` on Stop (PV-017) and is not persisted.

**PV-041 Minimum window.** `minZoom = 0.5 s / (1 s + groupDuration)`, which keeps at least about half a second
visible. — `GetMinZoom L3999–L4001` (constants from the binary)

**PV-042 Normalisation (`SetupZoomBar`).** — `SetupZoomBar L4040–L4084`
1. Swap `zoomL` and `zoomR` if they are reversed.
2. If the width is below `minZoom`, re-centre with width `minZoom`.
3. Keep the width when shifting: if `zoomR > 1`, shift the window left to end at 1; if `zoomL < 0`, shift it right
   to start at 0; finally clamp `zoomR ≤ 1`.
4. Update the scrollbar:
   * `page = max(1, (seekWidth − 2·border) div 10)`;
   * `max = round(page / width) − 1`;
   * `pos = round(max·zoomL)`.

   The scrollbar is disabled when `max ≤ 1` or `max ≤ page` (that is, not zoomed). Change events are ignored while
   the scrollbar is being set.
5. Update the map's zoom-window markers (PV-049).

**PV-043 Zoom buttons and menu (Ctrl+= / Ctrl+−).** The handler takes `zoom = |zoomR − zoomL|`. Zoom In makes
`zoom /= zoomK`; Zoom Out makes `zoom *= zoomK`. The new window is **centred on the current position**
`lastPosition`: `[lastPosition − zoom/2, lastPosition + zoom/2]`. Then normalise (PV-042) and redraw (`FormResize`).
Called without a sender, the handler only normalises and redraws. — `ButtonZoomInClick L4006–L4020`

**PV-044 Scrollbar drag.** `zoomL = (Position − Min)/(Max − Min)` and `zoomR = zoomL + previous width`. If the
scrollbar is disabled, the window becomes `[0,1]`. Then update the map markers and redraw.
— `ScrollBarZoomChange L4089–L4103`

### 3.7 A–B selection and loop

**PV-045 Set A (Ctrl+Home).** `selectL = GetCurrentPosition`. If `selectR ≤ selectL`, then `selectR = 1.0`.
Redraw. — `N_View_Time_SetAClick L4023–L4027, L4033`

**PV-046 Set B (Ctrl+End).** `selectR = GetCurrentPosition`. If `selectR ≤ selectL`, then `selectL = 0`.
Redraw. — `N_View_Time_SetAClick L4028–L4033`

**PV-047 Loop toggle (Ctrl+L, menu, or `CheckBoxLoop`).** The menu item and the checkbox mirror each other. When the
loop ends up checked while `selectR ≤ selectL`, the message `Cropping_Info` is shown: "Please, at first, select
fragment by set A-B points in current positions". The loop is unchecked on Stop. The selection itself is used
by playback (PV-022), by the seek-bar highlight (PV-032) and by video export (out of scope).
— `N_View_Time_LoopABClick L4132–L4141`, `ClearAll L1166–L1168`

**PV-048** The A–B selection is reset to `[0,0]` on Stop and is not persisted (`ClearAll L1186–L1187`).

**PV-049 Map "A/B" markers show the *time-zoom window*, not the selection.** `DrawMarkersAB` places map markers at
the GPS positions for `zoomL` (only if `zoomL > 0`) and `zoomR` (only if `zoomR < 1`). It skips points equal to the
track's first or last point. If only one of them is valid, it is used for both. When neither is valid, the markers
are cleared. The function is called from `SetupZoomBar` and `ScrollBarZoomChange`. — `DrawMarkersAB L1883–L1908`

### 3.8 Play speed

**PV-050 Speed buttons.** `PanelPlaySpeed` holds 6 radio speed buttons (`GroupIndex=1`, `AllowAllUp=False`) of
equal width `PanelPlaySpeed.Width div 6`, with tags **−3, −1, 0, 3, 15, 63**. The button with tag 0 is down by
default.
* Rate for tag `d`: `d ≥ 0` → `d + 1`; `d < 0` → `1/(1 − d)`. The resulting set is **¼, ½, 1, 4, 16, 64**.
* Label: `x(d+1)` for `d ≥ 0` (shown as "1x", "4x", …) and `1/(|d|+1)` for `d < 0`.
* The label is drawn into the glyph in Arial (or the language's `LangFont`) at 7 pt bold.

— `CreateSpeedButtons L658–L683`, `SetupSpeedBtn L1054–L1068`

**PV-051 GOP-dependent labels.** When a file is selected, if the first file's GOP size is a multiple of 15, the
last three buttons are retagged to **2, 14, 59** (rates **3, 15, 60**). Otherwise they are retagged to 3, 15, 63
(rates 4, 16, 64). — `ListViewSelectItem L2611–L2619`

**PV-052 Changing speed.** When the user clicks a speed button (and the media is seekable):
1. Remember `p = GetCurrentPosition`.
2. Pause through the Play toggle, then stop the decoder.
3. Apply the rate. With a DirectShow graph: if the rate equals the current one, first set 0.5 (or 1.0) to force a
   change, then set the rate. With the internal stream: the graph rate is 1 and the source group re-times the
   stream (`TSourceGroup.SetSpeed`).
4. Pause, `Seek(p, callTimer)`, restore the previous Play state.

The speed survives file changes in the session (RenderFile reapplies it, L2410) but is **not persisted**.
— `TrackBarPlaySpeedChange L3900–L3948`; UnitSources: `TSourceGroup.SetSpeed L678–L691`

**PV-053** `PanelPlaySpeed` is disabled until a file is selected and is disabled again by Stop (PV-010, PV-017).

### 3.9 Volume

**PV-055** The trackbar value `v` ∈ 0..100 maps to the player's linear volume `v·100` (DSPack `LinearVolume=True`,
so the gain is `v/100`). `v` is persisted as registry value `volume`, read at start-up with default 100. The value
is **deleted** when `v = 100`. The volume is re-applied every time the player is activated (PV-013). It is halved
temporarily during WaitForFrame (PV-007). The vertical-orientation branch (inverted value) is dead code.
— `TrackBarVolumeChange L1554–L1563`, `FormCreate L984`

### 3.10 Jumps and frame stepping

**PV-056 Jump back/forward (Left/Right arrows, menu, wheel).** Ignored while a frame wait is in progress. Requires
`p = GetCurrentPosition ≥ 0` and a seekable medium.
* Step `d = ±8.312 s` (constant from the binary) of **group wall-clock time**: `p += d / groupDuration`, applied only
  if `groupDuration > 0`. Clamp `p` to [0,1].
* Then `Seek(p, callTimer)`, `WaitForFrame(1000 ms)`, and repaint the full-screen form if it is visible.
* The play state is unchanged: a running video keeps running, a paused video shows the new frame.

Note that Left/Right are **menu shortcuts**, so they always jump, even when the file list has focus.
— `N_Playback_LeftClick L3164–L3189`

**PV-057 Next frame (N, button, wheel-down when paused).** Ignored while a frame wait is in progress.
1. If playing, pause (release `ButtonPlay`). If the state is still Running, set it to Paused.
2. Ask the renderer to step exactly **one frame** (`IVideoFrameStep.Step(1)`) and set `bStepMode`.
3. Wait at most 1 s while the state is Running.
4. Run one UI tick.

There is no explicit seek: the position is read back from the player. — `ButtonFrameNextClick L4852–L4879`

**PV-058 "Back Frames" (B, button, wheel-up when paused).** Ignored while a frame wait is in progress.
1. Pause as in PV-057.
2. Seek **backwards by 100 ms** with the internal source filter, or **by 1000 ms** with a DirectShow splitter:
   `Seek(lastPosition − ms/groupDuration, callTimer)`. It uses the cached `lastPosition`.
3. `WaitForFrame(1000 ms)`.

This is a coarse step back, not a single-frame step. — `ButtonFramePrevClick L4882–L4897`

### 3.11 Video window: zoom, pan, flips

**PV-060 Video zoom state.** It consists of:
* `videoZoom` (factor, default 1.0);
* the pan centre `videoZoomDx`, `videoZoomDy` (0..1, default 0.5);
* the mode radio group: Custom / Letterbox / PanAndScan / Stretch / NotLinear;
* the VMR-only flips FlipV and FlipH.

The mode and flips are menu items, persisted as described in PV-090. `videoZoom` and pan are not persisted and are
reset on Stop. If Custom is active at Stop, the mode reverts to Letterbox. — `ClearAll L1188–L1192`, `FormCreate L964–L967`

**PV-061 Re-layout entry point `ButtonVideoZoomOutClick(sender)`.** It runs on:
* the zoom buttons and menus;
* every `FormResize`;
* entering or leaving full screen;
* every file switch inside Seek;
* mode changes;
* each pan step;
* releasing the mouse in Custom mode.

`size` is the native picture size (for VMR, display-aspect corrected: `cx = cy·arX/arY`).
— `ButtonVideoZoomOutClick L4195–L4371`

With a sender (a user zoom action):
1. If Custom is **not** active yet, start from the largest power of `zoomK` (normally 2, or 1.2 from the wheel) at
   which the picture fits inside the panel (`z·cx ≤ W+1` and `z·cy ≤ H+1`). `prevZoom` is 0 in this case.
   Otherwise `prevZoom = videoZoom` (L4218–L4228).
2. Zoom Out divides by `zoomK`, but only if Custom was already active. Zoom In always multiplies by `zoomK`
   (L4231–L4235).
3. Select Custom (L4238).
4. **Snap back to Letterbox:** if `prevZoom > 0` and the "fit" factor `min(W/cx, H/cy)` lies strictly between
   `prevZoom` and the new factor, select Letterbox. Zooming through the fit size therefore returns to fit mode
   (L4240–L4243).
5. If the sender is a preset item whose caption contains `%` ("800%" … "10%"), set `videoZoom = N/100`
   (L4247–L4250).

Always:
1. Clamp `videoZoom` to **[0.02, 16]** (2 %–1600 %; constants from the binary) (L4253).
2. Show " (NN%)" on the Custom caption while Custom is checked (L4255).
3. If the native size or the panel size is unknown, the video window fills the panel (L4257, L4366–L4367).

**PV-062 Picture rectangle per mode** (panel client rect `L,T,R,B`, `W = R−L`, `H = B−T`):
* **Letterbox / NotLinear / Stretch:** the picture rect is the whole panel. The renderer keeps the aspect ratio for
  Letterbox, keeps it in the centre only for NotLinear (EVR non-linear stretch), and ignores it for Stretch (L4295–L4299, L4198–L4203).
* **PanAndScan ("cover"):**
  * If `cx/cy ≤ W/H`: picture width = `W`, height = `W·cy/cx`, centred vertically (top and bottom cropped).
  * Otherwise: height = `H`, width = `H·cx/cy`, centred horizontally (L4275–L4292).
* **Custom:**
  1. The picture size is `z·cx × z·cy`.
  2. Clamp `Dx` to `[b, 1−b]` with `b = min(0.5, (W/2)/(z·cx))`, and clamp `Dy` likewise using `H` and `cy`.
     A picture larger than the panel then can never expose the background, and a smaller one stays centred.
  3. The picture spans `[cxPanel − (1−Dx)·z·cx, cxPanel + Dx·z·cx]` horizontally, where `cxPanel` is the panel
     centre, and likewise vertically.

  (L4259–L4273, `GetD L4178–L4183`)

**PV-063 Display and source rectangles.** — L4302–L4363
* Round the picture rect and store it as `lastVideoPictureRect`.
* The display rect is the panel rect intersected with the picture rect (clamped side by side when the picture is
  smaller in either dimension). Store it as `lastVideoDisplayRect`.
* The video window is moved and sized to the display rect.
* The source rect is the normalised visible part of the frame:
  * `left = clamp01((disp.L−pic.L)/(pic.W−1))`
  * `top = clamp01((disp.T−pic.T)/(pic.H−1))`
  * `right = clamp01((disp.R−pic.L−1)/(pic.W−1))`
  * `bottom = clamp01((disp.B−pic.T−1)/(pic.H−1))`

The renderer draws `source rect → full display window`. **Flips (VMR only):** FlipV swaps the output top and bottom
(1 → 0), and FlipH swaps left and right.

**PV-064 Mode menu items.** Letterbox, PanAndScan, Stretch, NotLinear, FlipV, FlipH and Custom are ignored when
disabled. Otherwise they check the item (radio items) or toggle it (flips), persist it, and re-lay-out.
* **NotLinear** is enabled only with the EVR renderer, and **FlipV/FlipH** only with VMR.
* If NotLinear is checked but unavailable, Stretch is selected instead (L2847–L2854).
* Disabled items get the postfix " (EVRonly)" or " (VMRonly)". The translation keys are missing in EN, so the key
  text is shown (L4708–L4710).

— `N_VideoZoom_LetterboxClick L4144–L4150`

**PV-065 Custom zoom input ("Other").** An input box titled "Setup custom zoom", prompt "Magnitude in percent:",
pre-filled with `videoZoom·100` (2 decimals). On OK: `videoZoom = value/100`, select Custom, re-layout. The value is
clamped by PV-061. Non-numeric input raises the application's error handler (see Q6). — `N_VideoZoom_OtherClick L4464–L4471`

**PV-066 Pan by dragging.** In Custom mode, a left mouse-down on the video (or the panel) starts a grab: it remembers
the screen cursor position (`videoZoomGet`). On each mouse move while grabbing, with `(Δx, Δy)` being the change in
screen cursor position:
* `Dx += Δx / pictureWidth`;
* `Dy += Δy / pictureHeight`;
* re-layout (the clamps from PV-062 apply).

The picture follows the cursor 1:1. Mouse-up ends the grab and re-lays out. Without Custom mode, there is no
dragging. — `VideoWindowWMRMouseDown L4374–L4377`, `VideoWindowWMRMouseMove L3138–L3146`, `VideoWindowWMRMouseUp L4386–L4391`

**PV-067 Other video mouse actions.**
* **Middle button** toggles full screen. The PauseOnClick guard does not apply.
* **Right button** opens `PopupMenuVideoWindow`.
* **Left button** pauses or pans as described in PV-020 and PV-066.
* **Double-click** toggles full screen, except when PauseOnClick is on (PV-071 below).

— `VideoWindowWMRMouseDown L4378–L4381`

**PV-068 Cursor auto-hide.** Every mouse move over the video restores the cursor (if it was hidden), hides the
details popup, and restarts `TimerMouseHide` (2 s). The timer is restarted only while `ButtonPlay` is down. When the
timer fires, the cursor over the video window is hidden. — `VideoWindowWMRMouseMove L3137, L3149–L3158`, `TimerMouseHideTimer L4474–L4480`

### 3.12 Full screen

**PV-070 Toggle (`N_FullScreenClick`; Alt+Enter, double-click, middle-click, menu).** — `N_FullScreenClick L2871–L2898`
1. **Guard:** if PauseOnClick is checked and the sender is a video surface (`PanelVideo` or a video window), do
   nothing. A double-click on the form background or the menu still works.
2. Return the timeline to the main window if it was shown in full screen (PV-073), then toggle and persist the
   `N_FullScreen` check.
3. **Entering:** show `FormFullScreen` and move `PanelVideo` into it, without a border.
   **Leaving:** hide the form, move `PanelVideo` back into `PanelLeft` with a single border, restore and activate
   the main window.
4. With EVR, when paused, do Play then Pause to repaint the frame.
5. Re-lay-out the video and run one UI tick.

**PV-071** Double-click toggles full screen only when PauseOnClick is off (see PV-020). Stop and file changes always
leave full screen (PV-017).

**PV-072 Full-screen geometry.** The form covers the bounds of the monitor that contains the main window. If the main
window, shrunk by 100 px on every side, does not fit inside that monitor **and** the EVR renderer is in use, the form
spans the whole virtual desktop. — UnitFormFullScreen: `TFormFullScreen.FormShow L49–L61`

**PV-073 Auto-show timeline in full screen.** On mouse move over the video while in full screen:
* `Y ≥ PanelVideo.Height − 3` (pointer at the bottom edge): move `PanelBottom` and its splitter into the full-screen
  form, docked at the bottom, so the video shrinks above it.
* `Y < PanelVideo.Height − 50`: move them back to the main window.
* Between those two lines nothing changes (hysteresis).

After a move, re-layout. — `VideoWindowWMRMouseMove L3118–L3134`

**PV-074 Full-screen form events.**
* Closing it (Alt+F4) **closes the application** (`FormClose L64–L66`).
* The mouse wheel is forwarded to the main handler (`FormMouseWheel L69–L71`).
* Shortcuts are forwarded to the main form (`FormShortCut L74–L76`), so Space, arrows, Esc and the rest work.
* Esc leaves full screen (PV-016).
* The UI tick keeps updating the seek bar while the full-screen form is active (PV-026).

### 3.13 Mouse wheel (`FormMouseWheel`)

**PV-075 Wheel routing.** `d = WheelDelta div 120`. If `d = 0` (sub-notch deltas from smooth wheels and touchpads),
the event is consumed and ignored; there is no accumulation. Otherwise the target is chosen by the control under the
mouse, tested in this order:
1. **`PanelVideo`:** video zoom with factor **1.2**. `d > 0` zooms in and `d < 0` zooms out, once per event,
   regardless of `|d|` (L5349–L5353).
2. **`PanelBottomGraphics`** (seek bar and graphs): time zoom with factor **1.5**. Wheel up zooms in and wheel down
   zooms out. The window is centred on the current position (PV-043) (L5354–L5358).
3. **`PanelZoomScroll`:** scroll the time window with `Position = clamp(Position − 10·d, 0, Max − PageSize)` (L5359–L5362).
4. **`PanelControl`**, if `ButtonPlay` is enabled:
   * while playing, wheel up jumps back 8.312 s and wheel down jumps forward (PV-056);
   * while paused, wheel up does "Back Frames" (PV-058) and wheel down does "Next Frame" (PV-057).

   (L5363–L5372)
5. **Map (`WebBrowser`):** map zoom around the arrow or the mouse. The map spec covers this (L5373–L5390).

— `FormMouseWheel L5343–L5392`, `Inside L5330–L5333`, `SignToObj L5335–L5342`

### 3.14 Keyboard

**PV-080 Menu shortcuts.** All menu shortcuts in §2.5 are global to the main window and the full-screen window. VCL
dispatches menu shortcuts before the focused control sees the key, so Space, Left, Right, B, N and Esc work even
while the file list has focus.

**PV-081 Extra keys (`ApplicationEventsShortCut`).** These apply when the active form is the main form or the
full-screen form. — L4411–L4445
| Key | Without Ctrl | With Ctrl |
|---|---|---|
| Num + (0x6B) or `=`/`+` key (0xBB) | Video Zoom In (factor 2), consumed | Re-mapped to Ctrl+`=` → **time** Zoom In |
| Num − (0x6D) or `-` key (0xBD) | Video Zoom Out, consumed | Re-mapped to Ctrl+`-` → time Zoom Out |
| Num * (0x6A) | Video zoom **100 %** (preset) | (same) |
| Num / (0x6F) | Toggle Letterbox ↔ PanAndScan: if Letterbox is checked, select PanAndScan, otherwise select Letterbox | (same) |
| A | — | Select all rows in the file list |

When any **other** form of the app is active, Esc closes that form.

**PV-082** Ctrl and Shift are tracked from raw key messages and used for list multi-selection (PV-011).
— `ApplicationEventsMessage L4394–L4406`

**Complete shortcut table for this area:**

| Shortcut | Action |
|---|---|
| Space | Play / Pause |
| Esc | Stop, or leave full screen |
| Alt+Enter | Full screen toggle |
| ← / → | Jump −/+ 8.312 s |
| B / N | Back Frames / Next Frame |
| Alt+S / Alt+W | Save frame / Save window |
| Ctrl+= / Ctrl+− (also Ctrl+Num+/−) | Time zoom in / out |
| Ctrl+Home / Ctrl+End | Set A / Set B |
| Ctrl+L | Loop A-B |
| Ctrl+J | Video files overlap time |
| Num+ / `=` · Num− / `-` | Video zoom in / out |
| Num* | Video zoom 100 % |
| Num/ | Letterbox ↔ Pan&Scan |
| Alt+1 … Alt+4 | Letterbox / Pan&Scan / Stretch / Non-linear |
| F9 / F10 | Flip V / Flip H (VMR only) |
| F8 | Details popup on/off (`N_View_Details`) |

### 3.15 Start-up and command line

**PV-085** `TimerOnStartTimer` runs once after the form is shown. — `TimerOnStartTimer L5098–L5192`
1. If EVR is selected but unusable, switch to VMR and persist the change.
2. Create the video window for the chosen renderer.
3. If files were passed on the command line: add them, wait up to 10 s for the preview, select row 0, press
   `ButtonPlay` and call `ButtonPlayClick`. The first file **auto-plays**.
4. Otherwise refresh the file list.

`/whatsnew` shows What's New, and `/del<path>` deletes a file.

### 3.16 Snapshots (`N_Playback_Save_Frame` Alt+S, `N_Playback_Save_Wind` Alt+W)

**PV-086** — `N_Playback_Save_FrameClick L3599–L3668`
1. Read the position. If playing, **pause**.
2. Save Frame with EVR: show `Mes_EVR_Screenshot` **once per session**. The text explains that EVR saves the frame
   only at its on-screen size, and that VMR gives full quality.
3. Process messages, wait 500 ms, process messages.
4. Capture the image:
   * **Save Frame:** the current decoded frame from the renderer (with EVR, the frame at its display size). Name
     prefix `Frame`, EXIF description `Video frame`.
   * **Save Window:** leave full screen first, then grab the main window's screen rectangle (`Left, Top, Width,
     Height`). Prefix `Window`, description `Screenshot`.
5. Compute the timestamp `dt = T(lastPosition) + timeZoneUserShift`. If `dt < 2.0` (no valid date), use the current
   time.
6. Build the default file name:
   `"<prefix> " + YYYY.MM.DD HH-MM-SS(dt) + [" (" + elapsed video time with ms, e.g. h-mm-ss.mmm + ")"] + ".jpg"`.
   * The bracketed part is added only when a file is loaded.
   * Every `:` is replaced by `-`.
   * Characters that are illegal in file names are filtered out.

   (UnitUtilsStr `DateTimeForFileToStr L292–L310`.)
7. Show a Save dialog with default extension `jpg`, filter "Jpeg Files (.jpg;.jpeg)", overwrite prompt.
8. Write a JPEG at **quality 96** with these EXIF fields:
   * `ImageDescription` = description + " of " + first word of the window caption + video time + date. The exact
     concatenation is partly lost (Q7).
   * `DateTimeOriginal` and its sub-seconds = `dt`.
   * GPS coordinates of `lastPosition`, if the fix is good.
9. Set the file's timestamp to `dt`.

### 3.17 Renderer, splitter and decoder menus (DirectShow — to be dropped)

**PV-087 Splitters and decoders.** Toggle or check the item, persist it, then **Stop** playback. The user must reopen
the file. — `N_Playback_Decoder_MSVideo_ForceClick L5674–L5677`

User-visible effect: the internal splitters (MOV and AVI, default on) make a whole group play as one continuous
stream. This gives seamless file transitions, a frame-back step of 100 ms instead of 1 s, and speed implemented by
re-timing.

**PV-088 Renderer switch.** — `N_Playback_Renderer_NULClick L2801–L2868`
1. Remember the position and whether the player was active.
2. Stop, wait 200 ms.
3. Recreate the video window (Simple / VMR / EVR) and update the availability of NotLinear and the flips (PV-064).
4. If the player was active: re-select the item, restore the position, open it and **leave it paused**.

**rvplayer:** drop these menus. Keep the paused-resume behaviour as the model for any future backend switch.

### 3.18 Misc

**PV-089 Video files overlap (Ctrl+J).** Stop, then show an input box: title "Video overlap time", prompt "Enter time
length in milliseconds:". The value is registry `trimTailMS` (default "0"). On OK, store it and reload the selected
item. It trims overlapping file tails when a group is built, which changes the `pStart`/`pEnd` ranges. The data
model is in the file-list spec. — `N_View_Movie_OverlapClick L5682–L5689`

**PV-090 Persistence of menu checks.** Every main-menu item's `Checked` state is stored under the registry key
`HKCU\Software\DATAKAM PLAYER\` (UnitReg initialization L152–L154). The value name is the item name (for example
`N_Playback_PauseOnClick`, `N_Playback_PlayNext`, `N_VideoZoom_Letterbox`, `N_VideoZoom_FlipV`,
`N_View_Refresh_Fast`, `N_View_TimePoints_5m`, `N_Playback_Renderer_EVR`).
* The state is written on every change.
* The value is **deleted** when it equals the DFM default.
* For radio items, the whole group is written.
* Start-up reads every item, using its DFM default as the fallback.

— `ReadRegMenu L587–L593`, `DoReg L2690–L2695`, `MenuItemSaveReg L2696–L2707`, `MenuItemClick L2710–L2721`

**PV-091 Time points (map).** The "Time Points" radio sets the route marker step: 1 min, 5 min or 20 min, while
"Don't show" uses 100 days, which in effect means none. Selecting it redraws the route and runs one UI tick. The map
spec covers the drawing. — `N_View_TimePoints_1mClick L5035–L5040`, `RedrawRoute L1776–L1785`

**PV-092 Subtitles / overlay.** The app does not draw any text over the video. `UnitSubtitles` and
`TFormSetupSubtitleTemplate` generate SRT subtitles only during video export (`UnitFormCropping`). They use a
template with placeholders `%y% %m% %d% %h% %mi% %s% %date% %time% %lat% %lng% %bearing% %speed% %track% %alt%`,
stored in registry `subtitleTemplate`. `FillCoordinatesTepmalte` (L5710–L5740, decompilation failed) fills the
coordinate-copy and marker-URL templates. **Both are out of scope here** and belong to the export and coordinates specs.

---

## 4. State model

### 4.1 States

| State | Description |
|---|---|
| **Empty** | `currentSubitemIndex = −1`, player inactive. Play and Stop disabled, speed panel disabled. Every consumer shows "no data". |
| **Selected** | Metadata and GPS loaded, `currentSubitemIndex = 0`. The player is not yet active. Play is enabled and Stop disabled. A seek-bar click records `lastSeekPos`. The UI tick reports that value (PV-005 rule 2), so the pin, map and gauges move to it, and the next open (PV-013) starts there. |
| **Paused** | The player is active and paused. `ButtonPlay` is up, `Timer` is off, the screen saver is allowed. Every UI refresh is event-driven. |
| **Playing** | The player is running. `ButtonPlay` is down and shows the pause glyph, `Timer` is on, the screen saver is blocked. |
| **Dragging** (sub-state of Paused or Playing) | `pinGet = true`. All consumers show `pinGetValue`. The video keeps playing or stays paused at its real position until release. |
| **At end** (sub-state of Playing) | `p = 1.0` with Playing still active. Nothing happens unless PlayNext (or a loop to B = 1.0) applies. |
| **Stepping** | `bStepMode`, after Next Frame. The next Seek does Play then Pause to leave step mode. |

### 4.2 Transitions

| From | Event | To |
|---|---|---|
| Empty or any other state | select a row (PV-010) | Selected. Stop is performed first, so the previous file is unloaded. |
| Selected | single click on the row (PV-011) | Paused at `lastPosition` (0) |
| Selected or Paused | double click or Enter (PV-012), Space (PV-015), Play button | Playing. The player is activated first if needed. |
| Playing | Space or Play button, frame step, Save frame, speed change (temporary) | Paused |
| Paused or Playing | Seek: click, drag release, jump, back-frames, loop | same state, new position |
| Playing | `p ≥ selectR` with loop on | Playing at `selectL` |
| Playing | end of a non-last file in a group | Playing the next file, gap skipped (PV-005) |
| Playing | `p = 1` and PlayNext | Stop → select the next row (wrapping) → Playing |
| any | Stop button, Esc (outside full screen), decoder/splitter change, `Ctrl/Shift` multi-select | Empty. Full screen is left, loop and A/B are cleared, time zoom and video zoom are reset. |
| Paused or Playing | renderer change | Paused at the same position |

### 4.3 Single source of truth

* The only "now" value is `p` from `GetCurrentPosition`, cached as `lastPosition`.
* The seek bar, graph cursors, map arrow and route recentring, speed/azimuth/G-sensor gauges and the details popup
  are all drawn from that one `p` in the same tick (PV-025, PV-026).
* Every time-based conversion goes through `p`: wall clock via `T(p)`, elapsed time via `p·groupDuration`, and file
  plus media time via the subitem ranges.
* Seeking is always expressed in `p`. Seek then converts to (file, media time), reads back the reachable position
  and stores it in `lastSeekPos`. While the player is not running or paused, that value is what is reported.
* **rvplayer guidance:** keep one `position: f64` (progress) in the app state. Derive everything from it each frame.
  Implement drag as a "virtual position" override that is committed on release. Report the decoder's reached
  position after a seek, not the requested one.

---

## 5. Open questions

1. **Q1 Rate compensation (PV-005 step 4).** With a DirectShow graph and rate ≠ 1, the reported progress is
   slowed by dividing by the rate. This looks like a workaround for a DirectShow quirk, or a bug: at 4× the cursor
   would move at ¼ of the video speed. rvplayer decodes itself, so it should probably report the true media position.
   This needs confirmation by running the original at 4× with "Use preinstalled (from DirectShow)" only.
2. **Q2 PlayNext condition `p == 1.0` exactly.** This relies on the player reporting exactly the duration at the end.
   rvplayer should use an end-of-stream event or a tolerance. The behaviour also wraps from the last row to the first
   row; check whether that is desired.
3. **Q3 Back Frames step.** It is 100 ms or 1000 ms, depending on the splitter, and is not a single frame. Should
   rvplayer reproduce 100 ms, which is the default internal-splitter behaviour, or implement a true previous-frame
   step?
4. **Q4 `ScrollBarZoom.Min = 1`.** It is never changed at runtime, while `SetupZoomBar` computes positions from 0.
   That makes the scroll mapping slightly inconsistent. Reimplement the intended mapping instead: the thumb
   represents `[zoomL, zoomR]`.
5. **Q5 Loop toggled from the menu without A/B.** The checkbox and the menu each run the warning check, so the
   `Cropping_Info` message may appear **twice** (the menu sets the checkbox, which fires its own click). Reproduce
   it once.
6. **Q6 Custom zoom input.** Non-numeric input in "Other" goes to the global exception handler. rvplayer should
   validate the input.
7. **Q7 EXIF description of snapshots.** The exact pieces of the 8-part concatenation (L3647) were lost in
   decompilation. The known parts are the description, " of ", the first word of the window caption, a video-time
   string and the date/time. The exact format of `TimeDurationToStr` (elapsed time with ".ms") also needs a quick
   check against the original.
8. **Q8 Arrow keys.** In the original they always jump (menu shortcuts), even in the file list, where they cannot
   be used for navigation. Confirm whether rvplayer should keep this, given that Up/Down still navigate the list.
9. **Q9 EVR's own window double-click** (UnitEVRWindow `TEVR.WindowProc L67–L81`) calls the full-screen toggle
   with no sender, which bypasses the PauseOnClick guard. Whether that window receives clicks depends on Windows
   window stacking. In practice, a double-click with EVR may toggle full screen even with PauseOnClick on.
10. **Q10 Refresh "Fast" with Yandex Maps 2** gives 200 ms instead of 100 ms (L3575–L3576). This is probably a
    map-provider performance workaround. Decide whether it is still relevant.
11. **Q11 UI updates when another form is active.** The seek bar and map are not updated while a dialog or another
    app form is the active form (PV-025, PV-026). Decide whether to keep this.
12. **Q12 Seek while Stopped but active.** With the player active but in the Stopped state (rare), a seek that
    changes file updates `currentSubitemIndex` without loading the file (PV-006 step 4). The original never reaches
    this state in normal use. rvplayer should always load.
