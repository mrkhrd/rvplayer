# Registrator Viewer 6.0.0.22: application shell, settings, menus and secondary forms

Behavioural specification for the Rust/egui reimplementation (`rvplayer`). This document is the source of truth for:
the program shell (start-up and shutdown), settings persistence, the translation mechanism, the complete main-menu map, and the small secondary forms (About, What's New, message box, Copy, Cropping/Save video, Cropping item, File property, Repair, Setup device, Update exe, Update firmware).

Citation format: `Unit: Class.Method Lnnn–Lmmm`. Line numbers are the original Pascal source lines recovered from the debug info (`/* Lnnn */` markers in `rv_analysis/out/decomp/*.c`). Translation keys are given as `Key` → "English text" (from `TRANSLATE_EN.TXT`).

Requirement ID prefixes used here:
- `SH-` shell, start-up and shutdown
- `ST-` settings persistence
- `TR-` translation
- `MN-` menus
- `SF-<FORM>-` secondary forms

---

## 1. Overview

The original program is a single-window Delphi 7 VCL application. Its window title is always **"DATAKAM PLAYER - Registrator Viewer"**, and the application title is "DATAKAM PLAYER". The original author's brand was "Registrator Viewer"; the build was re-branded for DATAKAM through translation-resource overrides (§4.4, TR-042). When playback stops, the title is reset to "DATAKAM PLAYER - Registrator Viewer", followed by `"  -  " + <model>` if a dash-cam model was recognised on a memory card (`ButtonStopClick L1226–1233`; `registratorModel`, §6.11). During playback the title shows file details; the playback spec owns that (`ButtonPlayClick L1076–1151`).

Architecture of the shell:

| Piece | Original | Role |
|---|---|---|
| Program | `RegistratorViewer.dpr` (unit `DATAKAMPLAYER`, L28–35) | Calls `Randomize`, then `Application.Initialize`, creates `TFormApplication` and then `TFormMain`, calls `FormMain.Show` and finally `Application.Run`. |
| `TFormApplication` | `UnitFormApplication` | Hidden helper form used only to give the main form its own taskbar button. It has no UI. |
| `TFormMain` | `UnitFormMain` | The main window. It owns the main menu, the three pop-up menus, all panels, the file manager, the auto-update checker and the HTTP file server. |
| Settings | `UnitReg` | Flat key/value store in the Windows registry. All values are stored as strings. |
| Translation | `UnitTranslate` + `TFormMain.DoTranslate` | Key → text tables embedded as RCDATA resources `TRANSLATE_*.TXT`. They also hold colours, map start position and brand contact data. |
| Secondary forms | one unit each | Created lazily by an accessor function (`FormAbout()`, `FormCropping()`, …; the global `Form<Name>F` stays nil until first use). They are non-modal (`Show`) unless noted. They are re-translated by `TFormMain.DoTranslate` when they exist. |

Each change of a setting is written to the registry at the moment it happens. Nothing is saved in bulk on exit.

---

## 2. Start-up and shutdown

### 2.1 Process start

- **SH-001** There is **no single-instance protection**: no mutex, no `FindWindow` and no IPC. Each launch opens a new independent window. Two instances share the same registry settings, and the last writer wins. (A search of all units for `CreateMutex`/`OpenMutex`/`FindWindow` finds nothing.)
- **SH-002** Program sequence (`DATAKAMPLAYER: initialization L28–35`):
  1. Seed the random generator.
  2. Initialise the application.
  3. Create `TFormApplication`. It becomes the VCL "main form", but it is never shown.
  4. Create `TFormMain`, which runs `FormCreate` (§2.2).
  5. Show `FormMain`.
  6. Enter the message loop.
- **SH-003** `TFormApplication.FormCreate` (`UnitFormApplication: TFormApplication.FormCreate L26–30`) does three things:
  - sets `Application.ShowMainForm := False`;
  - clears its own owner window;
  - makes the hidden VCL application window owned by this helper form.

  Together with SH-005, the visible main window gets a normal taskbar button. **Rust:** no equivalent is needed. Use one top-level window.
- **SH-004** Unit initialisation sets the registry root (§3.1). It also prepares the translation tables, which are loaded in `FormCreate` step 3.

### 2.2 `TFormMain.FormCreate` (UnitFormMain: TFormMain.FormCreate L858–1026), in order

1. **L859** Name the GUI thread "Main". This is for debugging only.
2. **L861** `SetFormParent(Self, nil)` (`UnitUtils: SetFormParent L211–234`) adds `WS_EX_APPWINDOW` and clears the owner, so the main form has its own taskbar button.
3. **L863** `fixIE` (L517–523) is a one-time fix. If the registry value `IEFix` is empty:
   - write `IEFix = "done"`;
   - if `HKCU\Software\Microsoft\Internet Explorer\Main\Disable Script Debugger` equals `"yes"`, set it to `"no"`.

   This exists because the embedded IE WebBrowser hosts the map. *Drop in Rust.*
4. **L865** `LoadLanguages` builds all language tables and picks the initial language (§4.2).
5. **L867** Accept dropped files on the main window (`DragAcceptFiles`). Dropped files are handled by `WMDropFiles` (L5047–5063): all dropped paths are collected and passed to `AddFiles` (L5068–5079). `AddFiles` shows the hourglass cursor, adds each path to the file manager and then refreshes the list.
6. **L869–874** Choose the default video renderer from the OS version:
   - major version ≥ 6 (Vista or later): check `N_Playback_Renderer_EVR`;
   - otherwise: check `N_Playback_Renderer_WMR`.

   This happens **before** the defaults snapshot in step 10, so the OS-dependent renderer counts as the "default" for registry pruning (§3.3).
7. **L876–888** Load images from RCDATA bitmaps:
   - speed image `Form_Main_speed.bmp`;
   - speed buttons Play, Stop, VideoZoomIn, VideoZoomOut, FrameNext, FramePrev, ZoomIn and ZoomOut (`Btn_*.bmp`);
   - four file-list state icons (`FileList_icon_key`, `_key_partial`, `_video_warning`, `_gps_found`).
8. **L889–894** Menu icons: every item of `MainMenu` and `PopupMenuListView` gets the bitmap resource named `<ItemName>.bmp`, if one exists. The bitmap is transparent (`LoadMenuBitmaps` L767–774). The following items reuse other bitmaps:
   - `N_FileList_Save` reuses the `N_File_Save_Video` bitmap;
   - `N_FileList_Repair` reuses the `N_File_Repair_Video` bitmap;
   - `N_Help_Site_My` and `N_Help_Site_Brand` both use `N_Help_Site.bmp`.
9. **L908** `ReadDafaultsMenu` (L577–583, `ReadDefaultsM` L567–576) walks `MainMenu` recursively. It records the **names of all items that are checked at this moment** in `defaultMenuCheckedItems`. That list is the DFM default state plus the renderer choice from step 6. It is used only to decide whether a menu value needs to be stored (§3.3).
10. **L909** `AddLanguages` (L533–546) fills `N_View_Language` with one radio item per loaded language:
    - caption: `"(" + LangID + ") " + LangName`, for example `(en) English`, `(zh-cn) 简体中文`;
    - name: `N_View_Language_<LangID with '-' replaced by '_'>`;
    - `OnClick` = `DoTranslate`;
    - the item for `currentLang` is checked.
11. **L910** `AddTimeZones` (L551–560) appends 25 radio items to `N_View_TimeZone`, after `N_View_TimeZone_Local` and separator `N_S17`, for offsets −12 to +12:
    - caption: `"GMT" + ("-" if offset < 0 else "+") + abs(offset)`, so offset 0 gives `GMT+0`;
    - name: `N_View_TimeZone_GMT<offset+24>`, giving names `…GMT12` to `…GMT36`;
    - `OnClick` = the handler of `N_View_TimeZone_Local`.
12. **L911** `ReadRegMenu` (L587–593) walks the whole main menu recursively, including the language and time-zone items just added. For every non-separator item it sets `Checked := regReadB(<ItemName>, currentChecked)`. Radio semantics apply while restoring, so a later checked sibling unchecks earlier ones.
13. **L913–914** Play and Stop buttons are made flat.
14. **L917–923** Reset runtime state: map not initialised, arrow-in-centre on, no pending zoom restore, no current sub-item, step mode off, EVR-screenshot warning not yet shown.
15. **L925** `CreateFileListMenuButtons` creates the toolbar of buttons over the file list, mirroring pop-up items. This is covered by the file-list spec.
16. **L926** Run `PopupMenuListViewPopup` once, so that pop-up item enabled/visible states are initialised.
17. **L928–941** Initialise file-list columns:
    - column 0 is widened by the width of column 5 minus 13;
    - columns 1–4, 6 and 7 get +2 px;
    - column 5 is set to 1 px;
    - the list-view window procedure is subclassed (`NewListViewProc`);
    - double buffering is turned on;
    - `RestoreFileListColumns` (L705–731) loads saved widths and order (`col<i>_w`, `col<i>_i`; see §3).
18. **L945–947** Colours from translation (§4.4):
    - list background = `colorListBG`;
    - form background = `colorMainBG`;
    - video panel = black.
19. **L949–956** Create the custom drawn indicators and draw their idle state:
    - seek bar image (`ReplaceSeekImage`);
    - speedometer, drawn with speed 0 and unknown values;
    - azimuth compass, drawn as unknown;
    - G-sensor, drawn with 10/10/10;
    - seek bitmap.
20. **L958–967** Initialise the timeline zoom:

    | Field | Value |
    |---|---|
    | K | 2 |
    | L | 0 |
    | R | 1 |

    There is no selection (A = B = 0). Initialise the video zoom:

    | Field | Value |
    |---|---|
    | videoZoom | 1.0 |
    | Dx | 0.5 |
    | Dy | 0.5 |

    `N_VideoZoom_100` gets `"                      *"` appended to its caption, marking 100 % as the native size.
21. **L968–970** No file manager yet. Ctrl and Shift are not pressed.
22. **L972** `BrowserH := regReadF("panel_Browser", 0.6)`.
23. **L975–978** Default geometry:
    - width = 7/8 of primary monitor width;
    - height = 3/4 of primary monitor height;
    - centred on the primary monitor.
24. **L982** `RestartAutoupdate(5000)` (L3192–3204) (re)creates the background `TAutoUpdateChecker` (`UnitServerInfo`) with a 5000 ms start delay. It is restarted with a 500 ms delay when the map page reports it is initialised and when the list of known GPS formats or models changes. See §6.12 (SF-NET-005/006).
25. **L984** Volume trackbar position := `regReadI("volume", 100)`.
26. **L986–998** Apply the restored menu states by calling these handlers with `Sender = nil`, which applies state without toggling:
    - `N_View_Refresh_SlowClick` sets the refresh timer interval;
    - `N_View_Measu_MetricClick` sets the unit system;
    - `N_View_TimeZone_LocalClick` sets the time shift;
    - `N_View_FileListClick` sets panel visibility and the map provider;
    - `N_View_Graphics_SClick` sets which graphs are shown;
    - `N_View_Profile_GPSClick` applies the camera profile;
    - `N_View_Coord_FlipXClick` sets the speed-recalculation flag.
27. **L1000–1003** Collapse the hidden "analytics" and "ads" web browsers to height 0 and make them silent.
28. **L1005** `CopyMainMenu` (L631–653) builds the **video-window pop-up** and the **graphics pop-up** from the main menu (§5.3).
29. **L1007–1009** Set the caption and application title to "DATAKAM PLAYER". Set the hint hide pause to 5000 ms.
30. **L1011** `CreateSpeedButtons` (L658–683) creates six play-speed buttons in `PanelPlaySpeed`:
    - tags −3, −1, 0, 3, 15 and 63;
    - equal widths of panel width / 6;
    - grouped, with "0" down by default;
    - `OnClick` = `TrackBarPlaySpeedChange`.

    This is covered by the playback spec.
31. **L1013–1015** Restore panel sizes:
    - if `PanelRight.Width > 0`: `PanelRight.Width := max(5, regReadI("panel_Right", designWidth))`;
    - `PanelBottom.Height := regReadI("panel_Bottom", designHeight)`.
32. **L1017** Restore the window bounds from `pos_Left`, `pos_Top`, `pos_Width` and `pos_Height`. The defaults are the geometry from step 23.
33. **L1018–1022** Keep the window on the work area of the monitor that holds it:
    - if `Left > workArea.Right − 100` or `Left + Width < workArea.Left + 100`, then `Left := workArea.Left`;
    - if `Top > workArea.Bottom − 100` or `Top < workArea.Top`, then `Top := workArea.Top`.
34. **L1024** `WindowState := regReadI("pos_State", 2)`. The default is **maximised**.
35. **L1026** `DoTranslate(nil)` applies all texts (§4.3).

- **SH-005** The Rust app must perform the equivalent steps in this order. The parts that can be dropped are steps 1, 3, the IE-specific parts of 27, and the Windows owner/taskbar plumbing. The semantic order that matters:
  1. load languages;
  2. snapshot defaults;
  3. add the dynamic language and time-zone items;
  4. restore checked states from settings;
  5. apply the states;
  6. restore geometry;
  7. translate.

### 2.3 First timer tick: `TimerOnStartTimer` (UnitFormMain: TFormMain.TimerOnStartTimer L5098–5192)

`TimerOnStart` has Interval = 1 ms and is enabled in the DFM. It runs once, after the window is visible. The steps in scope here:

- **SH-010** **L5099** If the main form is not visible yet, do nothing; the timer will fire again. Otherwise disable the timer (L5102).
- **SH-011** **L5104–5105** Collapse the ads and analytics browsers again.
- **SH-012** **L5110–5121** Renderer sanity check. If EVR is selected, build a test graph with the EVR filter. If EVR cannot be instantiated:
  - switch to the WMR renderer;
  - persist both radio items through `MenuItemSaveReg`;
  - tear the test graph down.

  Then apply the renderer through `N_Playback_Renderer_NULClick(nil)`. *Video-backend specific; the playback spec decides.*
- **SH-013** **L5123–5128** Default copy folder. If `copyFolder` is empty:
  - use the user's *My Videos* folder (CSIDL 0x0E);
  - if that is empty, use *My Documents* (CSIDL 0x05);
  - write the result to `copyFolder`.
- **SH-014** **L5150–5163** Installation ID:
  1. Read the legacy value `HKCU\Software\Registrator Viewer\id`.
  2. If the legacy value is empty, read the own `id`.
  3. Otherwise, if the legacy value differs from the own `id`, write the legacy value into the own `id` (migration from the pre-DATAKAM install).
  4. If there is still no ID, generate 11 random letters, each case chosen at random, for example `kQzrTbaWmPe`. Store it and read it back.
  5. Put the result in `globalID`.
- **SH-015** **L5167–5168** `globalGPSFormats := regRead("gps")` and `globalGPSFormatsChanged := false`. `gps` is a comma-separated list, appended without duplicates (case-insensitive check) by `UnitUtils: AddGPSFormat L376–382`. It holds two kinds of entry: the format IDs of GPS logs the parser has recognised (`UnitGPSManager` L2432, L2768), and dash-cam model codes such as `GS8` or `G8M` recognised from a memory card's `autorun.inf` (`UnitFormUpdateFirmware: CheckFlashForRegistrator`, §6.11). ID and formats are only used as telemetry query parameters (`&gps=…&did=rw-<id>`, `UnitUtils: AddUrlAddons L387–398`) on the update-server URL. *They may be dropped.*
- **SH-016** **L5170–5183** Command-line files:
  1. Collect **all** `ParamStr(1..ParamCount)` (`GetParameterFiles` L5084–5088). Switches are not filtered out (SH-018).
  2. If any were collected, create the file manager (callback `OnFileManagerChanges`; GPS search on unless the profile is "video only") and add every argument as a path.
  3. Wait up to 10 s for preview information, then repaint the list.
  4. If the list is not empty, select item 0, press the Play button and start playback (`ButtonPlayClick`).
- **SH-017** **L5185–5186** If there is no file manager or it holds no files, call `N_FileList_RefreshClick`. This is the normal start-up auto-scan; see the file-list spec.
- **SH-018** **L5188–5191** Command-line switches:
  - `ParamStr(1) == "/whatsnew"` (exact, case-sensitive) opens the What's New window. The installer or updater passes it after an update.
  - If `ParamStr(2)` starts with `/del`, the rest of that argument is a file path, which is deleted silently. The self-updater uses this to remove the old executable; see the Update forms.

  Both switches are also fed to the file manager in SH-016. It ignores non-existent paths, so nothing visible happens. The Rust app should filter switches out before treating arguments as files.

### 2.4 Shutdown

- **SH-020** `N_File_Exit` (Alt+F4) calls `Close` (`N_File_ExitClick L1534–1536`).
- **SH-021** `FormCloseQuery` (L1030–1049) always allows the close. In order it:
  1. hides the window first, so that no further `pos_*` writes happen (§3.2);
  2. requests application termination;
  3. hides the seek-bar hover info;
  4. stops the file-manager worker;
  5. stops the auto-update checker;
  6. stops playback (`ButtonStopClick`);
  7. destroys the main menu, the video pop-up and the list pop-up, so that late events cannot reach them;
  8. disables drag-and-drop;
  9. frees the auto-update checker and the file manager.

  **No settings are written at close.** Everything is persisted when it changes.
- **SH-022** Unhandled exceptions (`ApplicationEventsException L5197–5208`):
  - exceptions of class `EDirectShowException` are swallowed silently;
  - any other exception, unless the application is already terminating, shows the custom message box (`rvShowMessage`, §6.3) with caption "Exception". The text is the exception message, a blank line and a call-stack dump.
- **SH-023** Keyboard pre-processing (`ApplicationEventsMessage L4394–4406`, `ApplicationEventsShortCut L4411–4445`):
  - The state of Ctrl and Shift is tracked from key-down/key-up messages.
  - When the active form is the main form or the full-screen form:

    | Key | Action |
    |---|---|
    | NumPad `+` or `=`, no Ctrl | Video zoom in, as the zoom-in button |
    | Ctrl + NumPad `+` | Rewritten to Ctrl+`=` so that it triggers timeline zoom in |
    | NumPad `-` or `-`, no Ctrl | Video zoom out |
    | Ctrl + NumPad `-` | Rewritten to Ctrl+`-` (timeline zoom out) |
    | NumPad `*` | Video zoom 100 % (`N_VideoZoom_100`) |
    | NumPad `/` | Toggles Letterbox ↔ Pan & Scan |
    | Ctrl+A | Select all files in the list |

  - When any **other** form is active, Esc closes that form. This is a global rule for all secondary forms.
- **SH-024** Settings reset (`N_Settings_ClearClick L5018–5032`):
  1. Ask a Yes/No confirmation: `Mes_ClearSettings` → "Are you sure to reset all application settings" + `"?"` + newline.
  2. On **Yes**, close the main form.
  3. Delete the whole registry key `HKCU\Software\DATAKAM PLAYER\`.
  4. Re-write `id` and `gps` from memory, so the install ID and known formats survive.
  5. Launch a new instance of the same executable with no arguments.
  6. Terminate.

  On **No**, do nothing.

---

## 3. Settings persistence (UnitReg)

### 3.1 Storage model

- **ST-001** All settings live in one registry key: **`HKEY_CURRENT_USER\Software\DATAKAM PLAYER\`**. Evidence: `regDefHKey` is initialised to `0x80000001` in the binary, and `regDefKey` is set by `UnitReg: initialization L152–153`.
- **ST-002** Every value is a **REG_SZ string**. The typed helpers convert:
  - `regWriteI`/`regReadI`: decimal integers. A read that fails to parse yields the default (`StrToIntDef`).
  - `regWriteB`/`regReadB`: the literal strings `"true"` / `"false"`. Anything else, or a missing value, yields the default (`UnitReg: regReadB L142–149`, `regWriteB L112–116`).
  - `regReadF`: `StrToExtended`, locale dependent. `panel_Browser` is written with a `.` decimal point (`FloatToStrPoint`, UnitFormMain L2531).
  - `regRead(name, def)`: returns `def` when the value is missing, but **an existing empty string is returned as empty**.
- **ST-003** Each call opens, reads or writes and closes the key; there is no caching. `regDel` deletes one value. `regDelAll` deletes the whole key (`UnitReg: regDelAll L88–95`).
- **ST-004** Rust: persist to a portable store, for example a TOML or JSON file in the config directory, with the same value names and string semantics, so that a migration importer can read the old registry key. Unknown or garbage values must fall back to the defaults exactly as above.

### 3.2 Settings table

The **Source** column cites where the value is read or written. "Other spec" means the detailed behaviour belongs to another writer; the name, type and default given here are authoritative.

| Value name | Type | Default | UI element / meaning | Written when | Source |
|---|---|---|---|---|---|
| `installer_language` | int (Windows LANGID, decimal) | `GetSystemDefaultLangID()` | Initial UI language before any explicit choice | Never written by the app; written by the installer | UnitTranslate: LoadLanguages L268 |
| `IEFix` | string | empty | One-shot flag for the IE script-debugger fix | First start (`"done"`) | UnitFormMain: fixIE L518–521 |
| `id` | string (11 letters) | generated | Anonymous installation ID, sent as `did=rw-<id>` to the update server | First start; migration; settings reset | TimerOnStartTimer L5150–5166; N_Settings_ClearClick L5026 |
| `gps` | string (comma list) | empty | GPS-log format IDs and dash-cam model codes ever detected (SH-015), sent as `gps=` | When a new format is first seen; settings reset | UnitUtils: AddGPSFormat L376–382; TimerOnStartTimer L5167 |
| `copyFolder` | string (path) | My Videos, else My Documents (`"C:\"` where read with an explicit default) | Last destination folder of Copy, Save fragment and Repair | First start; on choosing a folder in FormCopy, FormCropping or FormRepair | TimerOnStartTimer L5123–5128; N_File_Save_VideoClick L4926; forms §6 |
| `pos_Left`, `pos_Top`, `pos_Width`, `pos_Height` | int (px) | centred, 7/8 × 3/4 of primary monitor | Main window *normal* (restored) bounds | Every window move or resize while visible (`WMWindowPosChanged L5002–5012`), using the normal-position rectangle from `GetWindowPlacement` | FormCreate L1017 |
| `pos_State` | int | `2` | Window state: 0 = normal, 2 = maximised. Minimised (1) is never stored | Same as above, skipped when minimised | FormCreate L1024; WMWindowPosChanged L5006–5007 |
| `panel_Right` | int (px) | DFM width of `PanelRight` (510); minimum 5 | Width of the right-hand panel (vertical splitter `SplitterRight`) | Splitter moved, only if width > 0 | FormCreate L1014; SplitterRightMoved L2527–2529 |
| `panel_Bottom` | int (px) | DFM height of `PanelBottom` | Height of the bottom panel | Splitter moved | FormCreate L1015; SplitterRightMoved L2530 |
| `panel_Browser` | float (string, `.` decimal) | `0.6` | Fraction of the left column given to the map browser | Splitter moved | FormCreate L972; SplitterRightMoved L2531 |
| `volume` | int 0–100 | `100` | Volume trackbar | On change; **deleted** when 100 | FormCreate L984; TrackBarVolumeChange L1557–1562 |
| `col<i>_w` | int (px) | DFM width of column *i* | File-list column width; *i* = original column index | Column resized or dragged; deleted when equal to the default | RestoreFileListColumns L713–718; ListViewColumnDragged L5274–5290 (other spec) |
| `col<i>_i` | int | *i* | File-list column order: the original column shown at display position *i* | Column dragged; deleted when equal to *i* | RestoreFileListColumns L722–726; ListViewColumnDragged (other spec) |
| `col_sort` | int | `2` | File-list sort column | Column header click | ListViewColumnClick L3417–3420 (other spec) |
| `col_sort_dur` | bool | `true` | File-list sort direction flag | Column header click | same |
| `gm_X`, `gm_Y` | string (float) | translation `gmStartX` / `gmStartY` | Last map centre (longitude / latitude) | Reported by the map page | WebBrowserBeforeNavigate2 L3255–3302 (other spec) |
| `gm_Z` | int | translation `gmStartZoom` | Last map zoom | same | same |
| `gm_L` | string | empty | Last map layer / type chosen in the map page | same | same |
| `gm_shiftX`, `gm_shiftY` | string (float) | `"0.000000"` | Coordinate fix: shift longitude / latitude (`N_View_Coord_ShiftX/Y` input box) | On OK in the input box | N_View_Coord_ShiftXClick L5613–5620; UnitGPSManager reads (other spec) |
| `mapPostAction` | string (JavaScript) | empty | `N_View_Maps_PostAction`: script run after the map has loaded | On OK | N_View_Maps_PostActionClick L5630–5659 (other spec) |
| `markerURL` | string | `http://www.openstreetmap.org/?mlat={y}&mlon={x}&zoom={z}` | `N_View_MarkerURLSetup` / `N_View_MarkerURLOpen` command template | On OK | N_View_MarkerURLSetupClick L5750–5765 (other spec) |
| `coordCopyTemplate` | string | `{y}, {x}\n{date} {time}\n` followed by `speed: {s}km/h, direction: {a}°, track: {p}m` (metric) or `speed: {s}mph, direction: {a}°, track: {p}mile` (British) | `N_View_Coord_CopySetup` / `N_View_Coord_Copy` template; `\n` means a new line | On OK | coordCopyDef L5809–5815; N_View_Coord_CopySetupClick L5820–5828 (other spec) |
| `trimTailMS` | int (ms) | `0` | `N_View_Movie_Overlap`: overlap trimmed at the end of each file | On OK | N_View_Movie_OverlapClick L5682–5691; UnitSources L485 (other spec) |
| `trimHeadMS` | int (ms) | `0` | Overlap trimmed at the start of each file. **No UI**; registry only | never | UnitSources L484 |
| `subtitleTemplate` | string | built-in template | Subtitle template (FormSetupSubtitleTemplate); deleted when "Default" is ticked | On OK | UnitFormSetupSubtitleTemplate L90–140 (other spec) |
| *menu item name*, for example `N_View_Profile_GPS` | bool | the DFM default (§3.3) | Checked state of every checkable main-menu item | On every toggle; **deleted when equal to the default** | MenuItemSaveReg L2696–2707; ReadRegMenu L587–593 |
| `N_View_Language_<id>` | bool | false | Explicit language choice; `<id>` = LangID with `-` → `_`, for example `N_View_Language_zh_cn` | On choosing a language | AddLanguages L543; DoTranslate L4580 |
| `N_View_TimeZone_GMT<n>` | bool | false | Fixed time-zone choice, n = offset + 24 (12…36) | On choosing a zone | AddTimeZones L557 |

Values owned by the secondary forms are listed in each form's section (§6) and collected in §6.0.

Legacy key read once (`TimerOnStartTimer L5150`): `HKCU\Software\Registrator Viewer\id`. Foreign key written once: `HKCU\Software\Microsoft\Internet Explorer\Main\Disable Script Debugger`. *Drop in Rust.*

### 3.3 Menu-state persistence (`MenuItemClick`, `MenuItemSet`, `MenuItemSaveReg`, `MenuGetDefValue`, `MenuItemCopy`, `MenuItemSetPostfix`)

- **ST-010** **Defaults snapshot.** `defaultMenuCheckedItems` is the list of item names that were checked right after DFM load and the OS-dependent renderer choice (FormCreate L908; ReadDefaultsM L572–574). `MenuGetDefValue(name)` returns true when the name is in that list (L2670–2676). Dynamically added items (languages, GMT zones) are added later, so their default is always false.
- **ST-011** **Toggle.** `MenuItemClick(Sender)` (L2710–2721) works as follows:
  - A radio item becomes checked, and its siblings with the same GroupIndex are unchecked.
  - Any other item has its checked state inverted.
  - Then `MenuItemCopy(item)` mirrors the change to the copies in the pop-ups, and `MenuItemSaveReg(item)` persists it.

  Almost every checkable menu handler starts by calling `MenuItemClick(Sender)`. When the handler is invoked programmatically with `Sender = nil`, nothing is toggled and only the effect is applied.
- **ST-012** **Programmatic set.** `MenuItemSet(item, value)` (L2724–2730) changes the item only when the state differs, then copies it to the pop-ups and persists it.
- **ST-013** **Persist.** `MenuItemSaveReg(m)` (L2696–2707):
  1. If `m` is not a main-menu item (a pop-up copy), resolve it to the original through its link (`Tag`).
  2. Ignore unnamed items.
  3. For a non-radio item, call `DoReg(m)`. For a radio item, call `DoReg` on **every sibling** in the same parent.
  4. `DoReg` (L2690–2695): if `Checked == default`, **delete** the value; otherwise write `"true"` or `"false"`.

  The registry therefore holds only deviations from the defaults.
- **ST-014** **Restore.** At start-up, `Checked := regReadB(name, current)` for every item (ReadRegMenu). A missing value keeps the default.
- **ST-015** **Copies.** Pop-up copies are linked both ways through `Tag`:
  - main item → video-pop-up copy;
  - copy → original;
  - graphics-pop-up copies point to the original only.

  `MenuItemCopy(m)` (L2736–2752) finds the partner of `m`, and the partner of that partner, plus any graphics-pop-up item linked to `m`. It copies to each of them: Caption, Enabled, Visible, AutoCheck, RadioItem, GroupIndex, Checked, OnClick, ShortCut, AutoHotkeys and Break (`MenuItemCopy(m, p)` L2755–2767). **Rust:** keep one model per command; the pop-ups render the same model, so no copying is needed.
- **ST-016** **Postfix.** `MenuItemSetPostfix(m, text, show)` (L2773–2783) removes any existing `" (…)"` suffix from the caption. If `show` is true it appends `" (" + text + ")"`. Then it calls `MenuItemCopy`. It is used to mark items that are unavailable with the current renderer:
  - `N_VideoZoom_NotLinear`: suffix `EVRonly`;
  - `N_VideoZoom_FlipV` and `N_VideoZoom_FlipH`: suffix `VMRonly`;
  - the suffix appears only while the item is disabled (DoTranslate L4708–4710).
- **ST-017** `GetRelatedMenuItem(obj)` (L2786–2795) returns the linked partner of a menu item, or nil. Handlers use it to accept clicks from either copy.
- **ST-018** `ClearAll` (L1156–1195) is the "reset player state" routine run when the current file changes or the list is cleared. It touches no settings:
  - clears the current item and the per-file moving info;
  - re-enables the screen saver;
  - leaves full screen;
  - unchecks Loop A–B;
  - clears the map path, the A/B markers and the other markers;
  - redraws the speed, azimuth and G-sensor indicators as idle;
  - redraws all graphs empty;
  - resets the timeline (L = 0, R = 1, no selection) and the video zoom (1.0, centre 0.5/0.5);
  - if Custom zoom was active, switches to Letterbox;
  - clears the route script and the route.

  The detailed behaviour belongs to the playback spec; it is listed here only because the task names it.

---

## 4. Translation (UnitTranslate, TFormMain.DoTranslate)

### 4.1 Resource and file format

- **TR-001** Translations are RCDATA resources embedded in the executable:
  - `TRANSLATE_<LL>.TXT`: one per language, 24 files;
  - `TRANSLATE_ALL.TXT`: shared by all languages;
  - `TRANSLATE_FIX_ALL.TXT`: brand override for all languages;
  - `TRANSLATE_FIX_<LL>.TXT`: brand override for one language; only `FIX_RU` exists.

  Rust: embed the same files, for example with `include_str!`. An optional external override directory is acceptable, but it is not original behaviour.
- **TR-002** Encoding is UTF-8. A leading UTF-8 BOM (EF BB BF) is stripped (`GetResource2 L220–223`).
- **TR-003** Line parsing (`TLang.Create L129–171`):
  1. The text is split at CR and LF characters, and **empty lines are skipped** (`UnitUtilsStr: Split`, flags 0).
  2. Each line is trimmed.
  3. **Key** = the leading run of characters up to the first TAB or SPACE, **lower-cased**. Lookups are therefore case-insensitive: `N_Playback_Renderer_NUL` matches `N_Playback_Renderer_nul`.
  4. Whitespace (TAB/SPACE) after the key is skipped. **Value** = the rest of the line.
  5. If the value starts with `"`, its first and last characters are removed. The closing quote is assumed to be present; there are no escapes.
  6. Every literal two-character sequence `\n` in the value becomes a CR LF line break.
  7. A line with an empty key or an empty value is ignored.
  8. There is no comment syntax.
- **TR-004** Special keys inside a language file:

  | Key | Meaning |
  |---|---|
  | `LangID` | Language code (`en`, `ru`, `zh-cn`, …). **A file without `LangID` is discarded.** |
  | `LangName` | Native display name (UTF-8) |
  | `LangCodePage` | ANSI code page used for legacy controls. Not needed in Rust. |
  | `LangCharSet` | GDI charset for fonts. Not needed in Rust. |
  | `LangFont` | Optional font face override (`SetupFont`). Not used by any shipped file. |
  | `LangSystemID` | Comma-separated decimal Windows LANGIDs that select this language automatically |

  Shipped languages, in resource order: bg, by, cs, de, en, es, fr, hu, it, ja (file `JP`), lt, lv, nl, no, pl, pt, ro, ru, sk, slo (file `SL`), ua, vi, zh-cn, zh-tw.
- **TR-005** **Value references.** A value of the exact form `%OtherKey%` is replaced, once at load time, by the translated value of `OtherKey` in the same language, with the normal fallback (TR-008). For example, `N_FileList_Repair "%N_File_Repair_Video%"` yields "Repair broken file" (LoadLanguages L251–252).
- **TR-006** Implementation quirk, *do not reproduce*: `TLang.Create` stores item *i* at slot *i* but finally truncates the array to the number of accepted lines (L134, L147, L166). If any non-empty line is rejected (key without value, or a whitespace-only line), the same number of entries **at the end** of the combined text are lost. The shipped files contain no such lines, so the intended behaviour is "keep every valid pair".

### 4.2 Language loading and selection

- **TR-007** **Discovery and merge order** (`LoadLanguages L227–243`):
  1. Enumerate RCDATA resources.
  2. A resource is a language file if:
     - its name is longer than 15 characters;
     - it starts with `translate_` (case-insensitive);
     - its 13th character is `.` or `-`.

     This matches `TRANSLATE_EN.TXT` and `TRANSLATE_ZH-CN.TXT`, but not `TRANSLATE_ALL.TXT` or `TRANSLATE_FIX_*`.
  3. For each language, the text parsed is the concatenation, in **priority order** (first match wins, TR-008):
     1. `TRANSLATE_FIX_<LL>.TXT`, if present;
     2. `TRANSLATE_FIX_ALL.TXT`;
     3. `TRANSLATE_<LL>.TXT`;
     4. `TRANSLATE_ALL.TXT`.

     The fix files therefore override the language file, which overrides the shared file.
  4. The language whose `LangID` is `en` becomes `baseLang`.
- **TR-008** **Lookup and fallback** (`Find L62–66`, `Translate L83–88`, `TranslateDef L72–77`):
  1. Look up `key` in the current language.
  2. If not found and a backup key was given, look up `backupKey` in the current language.
  3. If not found and current ≠ English, look up `key` in English. The backup key is *not* tried in English.
  4. If still not found:
     - `Translate`/`TranslateW`/`TranslateA` return **the key itself**;
     - `TranslateDef` returns the supplied default, which is often empty.

  The variants differ only in the encoding returned (language code page, UTF-8 or system ANSI). Rust: a single UTF-8 `tr(key)`, `tr_or(key, backup)` and `tr_def(key, default)`.
- **TR-009** **Initial language** (`LoadLanguages L267–276`):
  1. Take `id := regReadI("installer_language", GetSystemDefaultLangID())`.
  2. The first language whose `LangSystemID` list contains `id` becomes current. If none matches, English is current.
  3. Then the menu restore (ST-014) may override this with a saved `N_View_Language_<id> = true`.
  4. `DoTranslate` (L4581–4585) finally sets `currentLang` to the index of the checked item in `N_View_Language`.

  Quirk: the Portuguese file lists `416,816`, which are hexadecimal LANGIDs written as decimal. Portuguese is therefore never auto-selected. *Rust: match 1046/2070. Note it as a fix.*
- **TR-010** **Switching language** happens through `View → Language → (xx) Name`. The handler is `DoTranslate` itself:
  1. The radio toggle is persisted (`MenuItemClick` L4580).
  2. `currentLang` is updated.
  3. All texts are re-applied immediately; no restart is needed.

  In Rust, also re-apply translated texts in any open secondary form.

### 4.3 What `DoTranslate` sets (UnitFormMain: TFormMain.DoTranslate L4576–4846)

`DoTranslate` runs only after `FormMain` exists (L4577). In order:

- **TR-020** **L4580–4585** Persist the language choice and set `currentLang` from the checked language item.
- **TR-021** **L4587–4629** Apply the language charset and font (`LangCharSet`, `LangFont`) to:
  - the default font, the hint, menu and icon fonts, and the main form font;
  - the font of every secondary form that exists.

  Rust: make sure the egui font set covers Cyrillic, Latin-Ext, Vietnamese and CJK (Simplified and Traditional, plus Japanese).
- **TR-022** **L4631–4639** Translate all menus with `TranslateMenu`: the main menu, the list pop-up and the graphics pop-up. The video pop-up is updated through the copies. The rule for each non-separator item (`TranslateMainMenuItem L4535–4570`):
  1. Items named `N_VideoZoom_<number>` (800, 400, 200, 100, 50, 25, 10) are skipped and keep their DFM captions (`800%` … `10%`).
  2. Caption = `TranslateW(item.Name)`.
  3. If that falls back to the key itself and the name ends in a digit, retry with the last character removed. This is how `N_FileList_Refresh2`, `N_FileList_Clear2`, `N_View_OpenStreetMaps2` and `N_View_YandexMaps2` get text: the first key has no entry, so the digit is stripped.
  4. Suffixes:
     - `N_FileList_Group_With` and `N_VideoZoom_Other` get `" . . ."` appended;
     - `N_VideoZoom_ZoomIn` gets `"   \t+"`;
     - `N_VideoZoom_ZoomOut` gets `"   \t-"`.

     The tab shows `+` / `-` in the shortcut column.
  5. Copy the item to the pop-up copies.
  6. Recurse into sub-items, **except** the submenus `N_View_Language` and `N_View_TimeZone`, whose children keep their generated captions.
  7. A cosmetic step pads items that have no icon so that captions align (L4553–4561). *Not needed in egui.*
- **TR-023** **L4641–4642** If the current language is not English, append `"   ( Language )"` to the `N_View_Language` caption. This lets a user find the menu in a language they cannot read.
- **TR-024** **L4644–4669** Composed captions:

  | Item | Caption |
  |---|---|
  | `N_Playback_Splitter_Int_MOV` | `N_Playback_Splitter_Intern` with `%format%` → `MOV/MP4`, giving "Use internal MOV/MP4 splitter" |
  | `N_Playback_Splitter_Int_AVI` | Same, with `AVI` |
  | `N_Playback_Decoder_MSVideo_{Force,Auto,Never}` | `N_Playback_Decoder_MSVideo` + `" - "` + `N_Playback_Decoder_{Force,Auto,Never}`, for example "Using Microsoft DTV-DVD Video Decoder - Auto" |
  | `N_Playback_Decoder_MSAudio_*` | Same, with `N_Playback_Decoder_MSAudio` |
  | `N_Help_Site_My` | `N_Help_Site` + `" Registrator Viewer"`, giving "Visit Site Registrator Viewer" |
  | `N_Help_Site_Brand` | `N_Help_Site` + `" "` + `"DATAKAM"`, giving "Visit Site DATAKAM". **Visible only if `MySiteBrand` translates to a non-empty value** (always true with the shipped `FIX_ALL`). |
  | `N_View_OpenStreetMaps` | `"[OpenLayers]:  "` + `N_View_OpenStreetMaps` + `" + OVI + Bing + ..."` |
  | `N_View_OpenStreetMaps2` | `"[Leaflet]:  "` + `N_View_OpenStreetMaps` + `" + OVI + Bing + Yahoo + ..."` |
  | `N_View_TimeZone_Local` (L4689) | `N_View_TimeZone_Local` + `" ("` + (`"+"` if the system offset > 0) + round(system offset in hours) + `")"`, for example "Local System (+3)" |
- **TR-025** **L4671–4672** File-list toolbar buttons get as hint the caption of the list pop-up item whose index equals the button's tag.
- **TR-026** **L4674–4688** File-list column captions, by original column index:

  | Index | Key | English |
  |---|---|---|
  | 0 | `FileList_Col_Name` | |
  | 1 | `FileList_Col_Duration` | |
  | 2 | `FileList_Col_DateTime` | |
  | 3 | `FileList_Col_Track` | |
  | 4 | `KmetersPerH` (metric) or `MilesPerH` (British) | |
  | 5 | `FileList_Col_FilePath` | |
  | 6 | `Mb` | |
  | 7 | `FileList_Col_PictSize` | |

  The file manager's cached display strings are then refreshed (`RefreshStrings`).
- **TR-027** **L4691–4707** Hints:
  - `TrackBarVolume` ← `Volume`;
  - `PanelPlaySpeed` ← `PlaySpeed`;
  - `CheckBoxLoop` caption ← `N_View_Time_LoopAB`, hint ← `N_View_Time_LoopABHint`;
  - `ImageAzimut` ← `Azimut`;
  - `ImageGSensor` ← three lines: `"x - " + N_View_Graphics_X`, `"y - " + …Y`, `"z - " + …Z`;
  - `ImageSpeed` ← three lines:
    - `"- " + N_View_Graphics_H + ", " + (Meters|Feets)`;
    - `"- " + Property_Track_Length + ", " + (Meters|Miles)`;
    - `"- " + Speed + ", " + (KmetersPerH|MilesPerH)`;
  - `ButtonVideoZoomIn` / `ButtonVideoZoomOut` ← the translated menu captions;
  - `ButtonFramePrev` / `ButtonFrameNext` ← menu caption + `" ("` + shortcut text (`B` / `N`) + `")"`.
- **TR-028** **L4708–4711** Apply the renderer postfixes (ST-016), then refresh the video zoom.
- **TR-029** **L4712–4714** Application title "DATAKAM PLAYER". Window caption "DATAKAM PLAYER - Registrator Viewer".
- **TR-030** **L4717–4844** Re-translate each secondary form that currently exists. This is the authoritative key map for those forms. Some forms also set a few texts themselves on create or show (see §6.1–6.12):

  **FormUpdateExe**

  | Element | Key |
  |---|---|
  | caption | `Update_Title` |
  | Label1 | `Update_NewVersion` |
  | Label2 | `Update_WantDownload` + `" ?"` |
  | ButtonAuto | `Update_Btn_Auto` |
  | ButtonBrowser | `Update_Btn_Browser` |
  | ButtonCancel | `Cancel` |

  **FormUpdateFirmware**

  | Element | Key |
  |---|---|
  | caption | `UpdateFirmware_Title`, backup `Update_Title` |

  **FormDetails** (other spec)

  | Element | Key |
  |---|---|
  | caption labels | `File:`, `SystemTime:`, `VideoTime:`, `Longitude:`, `Latitude:`, `Altitude:`, `Speed:`, `Azimut:`, `Details_Track_Length:` |
  | unit strings | `Meters`, `Miles`, `Feets`, `KmetersPerH`, `MilesPerH` |

  **FormGPSLog** (other spec): `FormShow` is re-run first.

  | Element | Key |
  |---|---|
  | caption | `GPSLog_Title` |
  | ButtonConvert | `GPSLog_Convert` |
  | CheckBoxAll | `GPSLog_SelectAll` |
  | CheckBoxShowPath | `GPSLog_ShowTrack` |
  | CheckBoxJoin | `GPSLog_JoinToOneFile` |
  | columns | `File`, `Points`, `FileList_Col_Duration`, `FileList_Col_Track` + `", "` + (`Kmeters`\|`Miles`) |
  | PopupMenuGPSLog | translated by item name |

  **FormGroup** (other spec)

  | Element | Key |
  |---|---|
  | caption | `Grouping_Title` |
  | ButtonGroup | `Grouping_Btn` |
  | ButtonClose | `Cancel` |

  **FormSetupDevice**

  | Element | Key |
  |---|---|
  | caption | `SetupDevice_Title` |
  | LabelDevice | `SetupDevice_Device:` |
  | LabelFrameSize | `PictureSize:` |
  | LabelTimeZone | `SetupDevice_TimeZone:` |
  | LabelGSensor | `SetupDevice_GSensor` |
  | ButtonWrite | `SetupDevice_BtnWrite` |
  | ButtonCancel | `Cancel` |

  **FormCropping**

  | Element | Key |
  |---|---|
  | caption | `Cropping_Title` |
  | ButtonStart | `Cropping_BtnStart` |
  | ButtonCancel | `Cancel` |
  | CheckBoxVideo | `Video` |
  | CheckBoxAudio | `Audio` |
  | CheckBoxMeta | `MetaInfo` + `" (GPS, G-Sensor)"` |
  | CheckBoxSRTSubtitles | `"(.srt) - "` + `Subtitles` |
  | GroupBoxVideoFileTracks | `Cropping_FileTracks` |
  | GroupBoxAdditionalFiles | `Cropping_AddonFiles` |
  | ButtonSubtitle | `SetupSubtitle_Title` |
  | ListView columns | `File`; `Started` + `" - "` + `Finished`; `Speed`; `Duration` |
  | PopupMenuCropList | translated by item name |
  | N_Speed | `Speed` |

  **FormCroppingItem**

  | Element | Key |
  |---|---|
  | caption | `Cropping_Title` + `". "` + `N_Edit` |
  | ButtonCancel | `Cancel` |
  | LabelStart | `Started:` |
  | LabelFinish | `Finished:` |
  | LabelDuration | `Duration:` |
  | LabelSpeed | `Speed:` |
  | SpeedUpMethod | `Cropping_SpeedUpMethod` |

  **FormSetupSubtitleTemplate** (other spec)

  | Element | Key |
  |---|---|
  | caption | `SetupSubtitle_Title` |
  | CheckBoxDefault | `Default` |

  **FormRepair**

  | Element | Key |
  |---|---|
  | caption | `Repair_Title` |
  | ButtonRepair | `Repair_BtnStart` |
  | LabelBroken | `Repair_BrokenFile:` |
  | LabelRef | `Repair_RefFile:` |
  | LabelResult | `Repair_FullOutputFile:` |
  | GroupBoxMethod | `Repair_Method` |
  | LabelCaptionFast | `Repair_MethodFast` |
  | LabelCaptionFull | `Repair_MethodFull` |

  **FormSelectFolder** (other spec)

  | Element | Key |
  |---|---|
  | caption | `SelectFolder` |
  | ButtonOk | `Ok` |
  | ButtonCancel | `Cancel` |
  | ButtonShowInTree | `Show` |
  | ButtonAddFolder | `CreateFolder` |

  **FormAbout**

  | Element | Key |
  |---|---|
  | caption | `N_Help_About` |
  | LabelMail hint | `SendMail` |
  | LabelSite hint | `N_Help_Site` |
  | LabelMail text | `MyEMail` |
  | LabelSite text | `MySiteBrand`, backup `MySite` |
  | LabelCity | `MyCity` |
  | LabelName | `MyName` |

  **FormWhatsNew**

  | Element | Key |
  |---|---|
  | caption | `N_Help_WhatsNew` + `" - "` + `"DATAKAM PLAYER"` + `" "` + version |

  In the tables above, `X:` means the translation of `X` followed by a colon.

### 4.4 Non-text entries: colours, map start, brand data

- **TR-040** Colour keys are read through `GetCustomColor(key)` (`UnitUtils: GetCustomColor L365–372`):
  - `#RRGGBB` gives an RGB colour;
  - a value starting with `cl` is a Delphi named colour (`clBtnFace`, `clWindow`, `clWindowText`, `clHighlight`, `clHighlightText`, …), which maps to system theme colours;
  - anything else is parsed as a decimal integer. If parsing fails, including a missing key, which returns the key name, the result is **255 (pure red)**.

  Keys and their EN values:

  | Key | EN value | Used for |
  |---|---|---|
  | `colorMainBG` | clBtnFace | Main form background |
  | `colorListBG` / `colorListText` | clWindow / clWindowText | File list |
  | `colorListSelBG` / `colorListSelText` | clHighlight / clHighlightText | File-list selection |
  | `colorListGrid` | #EEEEEE | File-list grid lines |
  | `colorGraphBG` | #FFFFFF | Graphs, Details window |
  | `colorGraphBorderT` / `colorGraphBorderB` | #666666 / #CCCCCC | Graph borders |
  | `colorGraphText` | #000000 | Graph text |
  | `colorGraphOS` / `colorGraphOSm` | #000000 / #EEEEEE | Graph axis / minor grid |
  | `colorGraph_S` | #0000FF | Speed curve |
  | `colorGraph_G` | #996E0B | G-sensor curve |
  | `colorGraph_H` | #000000 | Altitude curve |
  | `colorGraph_X` / `_Y` / `_Z` | #FF0000 / #008000 / #D300A9 | Acceleration curves |
  | `colorSpeedArrow` / `colorSpeedBG` / `colorSpeedText` | #FF0000 / #FFFFFF / #000000 | Speedometer |
  | `colorAboutBG` | #FFFFFF | About window background |

- **TR-041** Map start position: `gmStartX` (longitude), `gmStartY` (latitude) and `gmStartZoom`. These are the defaults for `gm_X`/`gm_Y`/`gm_Z`.
  - EN: −74.005767 / 40.713003 / 14 (New York).
  - `FIX_RU` overrides them for Russian to 37.62 / 55.75 (Moscow).
- **TR-042** Brand and contact keys (About window and Help menu):

  | Key | Value | Source file |
  |---|---|---|
  | `MyEMail` | `help@datakam.com` | FIX_ALL; RU: `info@datakam.ru` |
  | `MySiteBrand` | `http://datakam.com` | FIX_ALL; RU: `http://www.datakam.ru` |
  | `MyCity` | "DATAKAM technology Co.Ltd., Taiwan" | FIX_ALL; RU: "DATAKAM Москва, Россия" |
  | `MyName` | "+86 (755) 2227-1965" | FIX_ALL; RU: "+7(495) 647-09-79" |
  | `MySite` | `http://www.registratorviewer.com/index_en.html` | ALL; languages may override |
  | `MyEMail` | `vadikus@mail.ru` | ALL; shadowed by FIX_ALL |

  `FIX_RU` also overrides `FileList_Col_Duration` to "Мин.".
- **TR-043** `langid` (the `LangID`) is appended to server URLs (update check, analytics page) as `langid=`.

### 4.5 Rust requirements summary

- **TR-050** Provide the same merge order, lookup and fallback, and key case-insensitivity.
- **TR-051** Persist the language as in §3.3, using the `N_View_Language_<id>` naming so that imported settings work.
- **TR-052** Re-translate live on language change, and also when the unit system changes (`N_View_Measu_MetricClick L5405–5410` calls `DoTranslate`, because column and hint units change).

---

## 5. Menus

### 5.1 Shortcut encoding

A Delphi `TShortCut` stores the virtual-key code in the low byte. Modifiers are +$2000 Shift, +$4000 Ctrl and +$8000 Alt. The shortcuts below are already decoded. Main-menu shortcuts are active whenever the main window has focus, including when focus is in the file list. Through the copies they are also active in the full-screen window (other spec).

### 5.2 Main menu (`MainMenu` in TFORMMAIN.dfm)

Column meanings:

| Column | Meaning |
|---|---|
| Item / key | Component name and translation key. The key is the item name; see TR-022 for digit stripping and overrides. |
| English | Text resolved from EN, after the DoTranslate compositions. |
| Short | Decoded shortcut. |
| Radio/Chk | `R<g>` = radio item, group *g* (GroupIndex) within its submenu; `C` = check toggle via `MenuItemClick`; `✓` = checked by default in the DFM. |
| Handler | Source line range in UnitFormMain. |
| Icon | `●` = a `<Name>.bmp` icon exists in RCDATA. |

Separators (`N_S*`) are shown as `—`. The **Spec** column names the spec responsible for detailed behaviour: Shell = this document, FL = file list, PB = playback/video, GM = graphs/map.

#### File (`N_File` → "File")

| Item / key | English | Short | Radio/Chk | Handler | Behaviour | Spec | Icon |
|---|---|---|---|---|---|---|---|
| N_File_Open | Open | Ctrl+O | – | N_File_OpenClick L1542–1549 | Opens the file dialog (multi-select; filter "DashCam Files (*.mov, *.mp4)", "Video Files", "Any Files") and adds the chosen files to the list | FL | ● |
| N_FileList_Refresh2 | Refresh File List | Ctrl+R | – | N_FileList_RefreshClick L4448–4459 | Rescans and refreshes the file list | FL | ● |
| N_FileList_Clear2 | Clear File List | Ctrl+N | – | N_FileList_ClearClick L5267–5269 | Clears the list | FL | ● |
| N_File_Save_Video | Save selected fragment | Ctrl+S | – | N_File_Save_VideoClick L4904–4944 | Opens the Cropping (export) window with the A–B selection of the current file; MN-010 | Shell | ● |
| N_File_Repair_Video | Repair broken file | – | – | N_File_Repair_VideoClick L4976–4978 | Opens the Repair window with no file preset; MN-011 | Shell | ● |
| — N_S1 | | | | | | | |
| N_File_Convert_GPS | Convert GPS Log | Ctrl+G | – | N_File_Convert_GPSClick L3969–3972 | Opens the GPS-log converter with all log files known to the file manager, then stops playback; MN-012 | Shell (entry) / FL (form) | ● |
| — N_S15 | | | | | | | |
| N_SetupDevice | Setup Device | – | – | N_SetupDeviceClick L5211–5213 | Shows the Setup Device window (non-modal); MN-013 | Shell | ● |
| — N_S2 | | | | | | | |
| N_Settings_Clear | Reset All Settings | – | – | N_Settings_ClearClick L5018–5032 | Confirms, wipes the settings and restarts (SH-024) | Shell | ● |
| — N_S14 | | | | | | | |
| N_File_Exit | Exit | Alt+F4 | – | N_File_ExitClick L1534–1536 | Closes the main window (SH-020) | Shell | ● |

#### View (`N_View` → "View")

| Item / key | English | Short | Radio/Chk | Handler | Behaviour | Spec |
|---|---|---|---|---|---|---|
| N_View_Language | Language (+ "   ( Language )" when not EN) | – | submenu | – | Filled at run time with radio items `(id) Name`; click = `DoTranslate` (TR-010) | Shell |
| N_View_Profile | Profile | – | submenu | | | |
| ├ N_View_Profile_GPS_G | My dash cam has GPS and G-Sensor | – | R0 ✓ | N_View_Profile_GPSClick L5454–5530 | Camera profile. Shows or hides the speed/azimuth/G-sensor indicators and adjusts default graphs and maps: GPS+G shows all; GPS only hides the G graphs; Video only selects "no map" and hides GPS UI. Disables `N_View_Maps` for video-only | GM |
| ├ N_View_Profile_GPS | My dash cam has GPS | – | R0 | same | (same handler) | GM |
| └ N_View_Profile_Video | My dash cam can record only video | – | R0 | same | (same handler); also the file manager's GPS search is off at next creation | GM |
| — N_S18 | | | | | | |
| N_View_FileList | File List | F2 | C ✓ | N_View_FileListClick L2999–3096 | Shows or hides the file-list panel and re-lays out the window | FL |
| — N_S11 | | | | | | |
| N_View_Maps | Maps | – | submenu | | | |
| ├ N_View_OpenStreetMaps2 | [Leaflet]:  Open Street Maps + OVI + Bing + Yahoo + ... | – | R0 ✓ | N_View_GoogleMapsClick L2989–2993 → N_View_FileListClick | Map provider 5, page `osm2.html` | GM |
| ├ N_View_OpenStreetMaps | [OpenLayers]:  Open Street Maps + OVI + Bing + ... | – | R0 | same | Provider 4, `osm.html` | GM |
| ├ N_View_GoogleMaps | Google Maps | F3 | R0 | same | Provider 1, `gm.html`. Pressing F3 while Google is already checked selects **None** instead (toggle) | GM |
| ├ N_View_YandexMaps | Yandex Maps (detailed for Russia) | – | R0 | same | Provider 2, `ym.html` | GM |
| ├ N_View_YandexMaps2 | Yandex Maps 2.0 | – | R0 | same | Provider 3, `ym2.html` | GM |
| ├ N_View_NavitelMaps | Navitel Maps (detailed for Russia) | – | R0 | same | Provider 6, `nm.html` | GM |
| ├ N_View_BaiduMaps | Baidu Maps (detailed for China) | – | R0 | same | Provider 7, `bm.html` | GM |
| ├ N_View_NoMaps | None | – | R0 | same | Provider 0, `about:blank` + analytics ping | GM |
| ├ — N_S20 | | | | | | |
| ├ N_View_Maps_PostAction | JavaScript: Do After Map Loaded | – | – | N_View_Maps_PostActionClick L5630–5659 | Input box; stored in `mapPostAction` | GM |
| └ N_View_Maps_OpenKML | Overlay vector image (KML) | – | – | N_View_Maps_OpenKMLClick L5845–5871 | Opens a KML/GPX file and overlays it via the local HTTP server | GM |
| N_View_Arrow | Arrow on Map | – | submenu | | | |
| ├ N_View_Arrow_Small | Small | – | R1 | N_View_Arrow_SmallClick L3583–3586 | Map arrow size; redraws via `TimerTimer` | GM |
| ├ N_View_Arrow_Normal | Normal | – | R1 | same | | GM |
| └ N_View_Arrow_Big | Big | – | R1 ✓ | same | | GM |
| N_View_TimePoints | Time Points | – | submenu | | | |
| ├ N_View_TimePoints_1m | 1 minute | – | R0 | N_View_TimePoints_1mClick L5035–5040 | Time labels along the route every N minutes; redraws the route | GM |
| ├ N_View_TimePoints_5m | 5 minutes | – | R0 | same | | GM |
| ├ N_View_TimePoints_20m | 20 minutes | – | R0 | same | | GM |
| └ N_View_TimePoints_Never | Don't show | – | R0 ✓ | same | | GM |
| N_View_Coordinates | Fix Coordinates | – | submenu | | | |
| ├ N_View_Coord_FlipX | Flip/Reflect X (Longitude) | – | C | N_View_Coord_FlipXClick L5602–5608 | Coordinate fix flag; stops playback so the track reloads | GM |
| ├ N_View_Coord_FlipY | Flip/Reflect Y (Latitude) | – | C | same | | GM |
| ├ — N_S23 | | | | | | |
| ├ N_View_Coord_ShiftX | Shift X (Longitude) | – | – | N_View_Coord_ShiftXClick L5613–5623 | Input box (title = `N_View_Coordinates` caption); stored in `gm_shiftX` | GM |
| ├ N_View_Coord_ShiftY | Shift Y (Latitude) | – | – | same | Stored in `gm_shiftY` | GM |
| ├ — N_S24 | | | | | | |
| └ N_View_Coord_Speed | Calculate Speed and Heading Only by Coordinates | – | C | N_View_Coord_FlipXClick | Sets the recalculate-speed flag | GM |
| N_View_UseCoordinates | Current Coordinates | – | submenu | | | |
| ├ N_View_MarkerURLSetup | Setup URL for Command | – | – | N_View_MarkerURLSetupClick L5750–5765 | Input box with examples; stored in `markerURL` | GM |
| ├ N_View_MarkerURLOpen | Execute Command for Current Point | M | – | N_View_MarkerURLOpenClick L5772–5803 | Expands the template and runs or opens it; a leading `@` means download silently | GM |
| ├ — N_S25 | | | | | | |
| ├ N_View_Coord_CopySetup | Setup Template for Copy | – | – | N_View_Coord_CopySetupClick L5820–5828 | Input box; stored in `coordCopyTemplate` | GM |
| └ N_View_Coord_Copy | Copy Current Point to Clipboard | C | – | N_View_Coord_CopyClick L5831–5839 | Copies the expanded template to the clipboard | GM |
| — N_S12 | | | | | | |
| N_View_ControlPanel | Control Panel | F4 | C ✓ | N_View_FileListClick | Shows or hides the control panel | PB |
| N_View_Refresh | Panels Refresh | – | submenu | | | |
| ├ N_View_Refresh_Slow | Slow | – | R0 | N_View_Refresh_SlowClick L3568–3580 | UI timer 1000 ms | PB |
| ├ N_View_Refresh_Norm | Normal | – | R0 | same | 300 ms | PB |
| └ N_View_Refresh_Fast | Fast | – | R0 ✓ | same | 100 ms, or 200 ms when Yandex 2.0 maps are active | PB |
| N_View_Measurement | Measurement | – | submenu | | | |
| ├ N_View_Measu_Metric | Metric system (meters, kilometers) | – | R0 ✓ | N_View_Measu_MetricClick L5405–5412 | Sets `bBritishSystem`; re-translates (TR-052) and re-lays out | GM |
| └ N_View_Measu_British | British system (feet, miles) | – | R0 | same | | GM |
| — N_S13 | | | | | | |
| N_View_Time | Timeline | – | submenu | | | |
| ├ N_View_Time_ZoomIn | Zoom In | Ctrl+= (VK_OEM_PLUS; also Ctrl+NumPad+, SH-023) | – | ButtonZoomInClick L4006–4020 | Timeline zoom in | PB |
| ├ N_View_Time_ZoomOut | Zoom Out | Ctrl+- (VK_OEM_MINUS; also Ctrl+NumPad−) | – | same | Timeline zoom out | PB |
| ├ N_View_Movie_Overlap | Set video files overlap time | Ctrl+J | – | N_View_Movie_OverlapClick L5682–5691 | Input box (`Mes_MovieOverlapTitle` / `Mes_MovieOverlapText`); stored in `trimTailMS` | PB |
| ├ N_View_Time_SetA | Set selection start (A) in current position | Ctrl+Home | – | N_View_Time_SetAClick L4023–4034 | Sets A | PB |
| ├ N_View_Time_SetB | Set selection end (B) in current position | Ctrl+End | – | same | Sets B | PB |
| └ N_View_Time_LoopAB | Loop A-B | Ctrl+L | – | N_View_Time_LoopABClick L4132–4141 | Toggles A–B loop (mirrors `CheckBoxLoop`) | PB |
| N_View_TimeZone | Time Zone | – | submenu | | | |
| ├ N_View_TimeZone_Local | Local System (±h) | – | R0 ✓ | N_View_TimeZone_LocalClick L5418–5436 | Display times in local time | PB |
| ├ — N_S17 | | | | | | |
| └ GMT−12 … GMT+12 (25 items, dynamic) | `GMT-12` … `GMT+12` | – | R0 | same handler | User shift = zone/24 − local offset (days); refreshes strings and layout | PB |
| N_View_Graphics | Graphics | – | submenu | | | |
| ├ N_View_Graphics_S | Speed | Ctrl+Alt+S | C ✓ | N_View_Graphics_SClick L2929–2968 | Shows or hides the speed graph | GM |
| ├ N_View_Graphics_H | Altitude | Ctrl+Alt+H | C | same | Altitude graph | GM |
| ├ N_View_Graphics_G | G-Sensor Value | Ctrl+Alt+G | C ✓ | same | G-sensor graph | GM |
| ├ N_View_Graphics_V | Acceleration Total Vector | Ctrl+Alt+V | C | same | | GM |
| ├ N_View_Graphics_X | Acceleration Left-Right | Ctrl+Alt+X | C | same | | GM |
| ├ N_View_Graphics_Y | Acceleration Forward-Backward | Ctrl+Alt+Y | C | same | | GM |
| ├ N_View_Graphics_Z | Acceleration Up-Down | Ctrl+Alt+Z | C | same | | GM |
| ├ — N_S9 | | | | | | |
| ├ N_View_Graphics_Hide | Hide All Graphics | F6 | – | N_View_Graphics_ShowClick L2973–2986 | Sets all graph items false | GM |
| └ N_View_Graphics_Show | Show All Graphics | F7 | – | same | Sets all true; G/V/X/Y/Z only if `N_View_Graphics_G` is visible | GM |
| N_View_Details | Details | F8 | C ✓ | N_View_FileListClick | Shows or hides the Details panel or window | GM |
| — N_S3 | (trailing separator) | | | | | |

Icons: `N_View_Movie_Overlap`, `N_View_Time_SetA`, `N_View_Time_SetB`, `N_View_Time_LoopAB`, `N_View_Time_ZoomIn` and `N_View_Time_ZoomOut` have icons.

#### Playback (`N_Playback` → "Playback")

| Item / key | English | Short | Radio/Chk | Handler | Behaviour | Spec |
|---|---|---|---|---|---|---|
| N_Playback_Play | Play / Pause | Space | – | N_Playback_PlayClick L3099–3104 | Toggles play/pause | PB |
| N_Playback_Stop | Stop | Esc | – | N_Playback_StopClick L3107–3112 | Stop | PB |
| N_FullScreen | Full Screen | Alt+Enter | C (state) | N_FullScreenClick L2871–2898 | Toggles full screen; also triggered by double-click on the form | PB |
| — N_S19 | | | | | | |
| N_Playback_Left | Jump Back | Left | – | N_Playback_LeftClick L3164–3189 | Seeks back | PB |
| N_Playback_Right | Jump Forward | Right | – | same | Seeks forward | PB |
| N_Playback_Frame_Prev | Back Frames | B | – | ButtonFramePrevClick L4882–4897 | Steps back | PB |
| N_Playback_Frame_Next | Next Frame | N | – | ButtonFrameNextClick L4852–4879 | Steps forward | PB |
| — N_S5 | | | | | | |
| N_Playback_PauseOnClick | Pause on Click | – | C | N_Playback_PauseOnClickClick L5216–5218 | Toggle only; used by the video-window click handler | PB |
| N_Playback_PlayNext | Auto Switch to Next File | – | C | N_Playback_PlayNextClick L5221–5223 | Toggle only | PB |
| — N_S4 | | | | | | |
| N_Playback_Save_Frame | Save Current Video Frame | Alt+S | – | N_Playback_Save_FrameClick L3599–3669 | Saves the frame as JPEG (`SaveDialogJpeg`) | PB |
| N_Playback_Save_Wind | Save Current Window | Alt+W | – | same | Saves a window screenshot | PB |
| — N_S6 | | | | | | |
| N_Playback_Splitters | Splitters | – | submenu | | | |
| ├ N_Playback_Splitter_Int_MOV | Use internal MOV/MP4 splitter | – | C ✓ | N_Playback_Decoder_MSVideo_ForceClick L5674–5677 | Toggle, then stop playback | PB |
| ├ N_Playback_Splitter_Int_AVI | Use internal AVI splitter | – | C | same | | PB |
| ├ — N_S22 | | | | | | |
| └ N_Playback_Splitter_DSShow | Use preinstalled (from DirectShow) | – | C ✓ | same | | PB |
| N_Playback_Decoders | Decoders | – | submenu | | | |
| ├ N_Playback_Decoder_MSVideo_Force | Using Microsoft DTV-DVD Video Decoder - Force | – | R1 | same | | PB |
| ├ N_Playback_Decoder_MSVideo_Auto | … - Auto | – | R1 ✓ | same | | PB |
| ├ N_Playback_Decoder_MSVideo_Never | … - Never | – | R1 | same | | PB |
| ├ — N_S21 | | | | | | |
| ├ N_Playback_Decoder_MSAudio_Force | Using Microsoft DTV-DVD Audio Decoder - Force | – | R2 | same | | PB |
| ├ N_Playback_Decoder_MSAudio_Auto | … - Auto | – | R2 ✓ | same | | PB |
| └ N_Playback_Decoder_MSAudio_Never | … - Never | – | R2 | same | | PB |
| N_Playback_Renderer | Renderer | – | submenu | | | |
| ├ N_Playback_Renderer_NUL | Simple | – | R1 | N_Playback_Renderer_NULClick L2801–2868 | Switches the video renderer and reopens the file | PB |
| ├ N_Playback_Renderer_WMR | VMR (Windows XP) | – | R1 (default on XP) | same | | PB |
| └ N_Playback_Renderer_EVR | EVR (Windows Vista / 7, hardware acceleration DXVA) | – | R1 ✓ (default Vista+) | same | | PB |

The Splitters, Decoders and Renderer submenus and all Playback leaf items except PauseOnClick and PlayNext have icons. Splitters, Decoders and Renderer are DirectShow-specific: the Rust app may hide them or replace them with backend options, but the **setting names must not be reused with other meanings**.

#### Zoom (`N_VideoZoom` → "Zoom"; all items GroupIndex 1)

| Item / key | English | Short | Radio/Chk | Handler | Behaviour | Spec |
|---|---|---|---|---|---|---|
| N_VideoZoom_Custom | Custom | – | R1 | N_VideoZoom_LetterboxClick L4144–4150 | Set automatically when a numeric zoom is in effect | PB |
| — N_S7 | | | | | | |
| N_VideoZoom_ZoomIn | Zoom In (shows "+") | (NumPad+ / =, SH-023) | – | ButtonVideoZoomOutClick L4195–4371 | Zoom in | PB |
| N_VideoZoom_ZoomOut | Zoom Out (shows "-") | (NumPad− / −) | – | same | Zoom out | PB |
| — N_S8 | | | | | | |
| N_VideoZoom_800 … N_VideoZoom_10 | `800%` `400%` `200%` `100%` + `"  …*"` `50%` `25%` `10%` | NumPad* = 100 % | – | same | Fixed zoom factor | PB |
| N_VideoZoom_Other | Other . . . | – | – | N_VideoZoom_OtherClick L4464–4471 | Input box `Mes_CustomZoomTitle` / `Mes_CustomZoomText` | PB |
| N_VideoZoom_Letterbox | (Letterbox) Fill in the window, keeping the aspect ratio | Alt+1 | R1 ✓ | N_VideoZoom_LetterboxClick | Fit | PB |
| N_VideoZoom_PanAndScan | (Pan and Scan) Stretch to entire window, keeping the aspect ratio | Alt+2 | R1 | same | Fill / crop | PB |
| N_VideoZoom_Stretch | Stretch to entire window, without keeping the aspect ratio | Alt+3 | R1 | same | Stretch | PB |
| N_VideoZoom_NotLinear | Not linear stretching, keeping the aspect ratio in the center (+ " (EVRonly)" when disabled) | Alt+4 | R1 | same | Non-linear | PB |
| — N_S10 | | | | | | |
| N_VideoZoom_FlipV | Flip Vertical (+ " (VMRonly)" when disabled) | F9 | C | same | Flip | PB |
| N_VideoZoom_FlipH | Flip Horizontal (+ " (VMRonly)" when disabled) | F10 | C | same | Mirror | PB |

`N_VideoZoom_LetterboxClick` ignores clicks from invisible items (L4145). ZoomIn and ZoomOut have icons.

#### Help (`N_Help` → "?")

| Item / key | English | Short | Handler | Behaviour | Spec | Icon |
|---|---|---|---|---|---|---|
| N_Help_About | About | F1 | N_Help_AboutClick L2901–2903 | Shows FormAbout (non-modal) | Shell | ● |
| N_Help_WhatsNew | What's New? | – | N_Help_WhatsNewClick L5226–5228 | Shows FormWhatsNew (non-modal) | Shell | ● |
| — N_S16 | | | | | | |
| N_Help_Site_Brand | Visit Site DATAKAM | – | N_Help_Site_BrandClick L5400–5402 | Opens `Translate("MySiteBrand")` in the default browser (`http://datakam.com`; RU `http://www.datakam.ru`) | Shell | N_Help_Site.bmp |
| N_Help_Site_My | Visit Site Registrator Viewer | – | N_Help_Site_MyClick L5395–5397 | Opens `Translate("MySite")` (`http://www.registratorviewer.com/index_en.html` or a language override) | Shell | N_Help_Site.bmp |

Translations and icons still exist for a removed **N_Help_Donate** ("Make Donation"), but there is no menu item. *Do not add it.*

### 5.3 Pop-up menus built from the main menu

- **MN-001** **Video-window pop-up** (`PopupMenuVideoWindow`, built by `CopyMainMenu L631–653`; it opens on right-click in the video area), in order:
  1. copies of every `N_Playback` child **except** the Splitters, Decoders and Renderer submenus;
  2. a separator;
  3. copies of the top-level menus **File**, **View** and **Zoom** as submenus. That is every top-level item except Playback and Help. The File copy omits `N_File_Exit`;
  4. a separator;
  5. `N_File_Exit`.

  Copies are linked to the originals (ST-015), so checks, enabled state and captions stay in sync.
- **MN-002** **Graphics pop-up** (`PopupMenuGraphics`; right-click on the graphs), in order:
  1. copies of the `N_View_Time` children except `N_View_Time_LoopAB` (ZoomIn, ZoomOut, Movie_Overlap, SetA, SetB);
  2. `N_File_Save_Video`;
  3. a separator;
  4. copies of all `N_View_Graphics` children;
  5. a separator;
  6. `N_View_Details`.

  Copies are named `<original>4` (CopyMenu L609).
- **MN-003** **File-list pop-up** (`PopupMenuListView`) is designed in the DFM. Its items and shortcuts:

  | Item | English | Shortcut | Handler |
  |---|---|---|---|
  | N_FileList_Open | Open | Enter | ListViewDblClick |
  | — | | | |
  | N_FileList_Add | Add Files | Insert | N_File_OpenClick |
  | N_FileList_Del | Remove from List | Del | |
  | N_FileList_Refresh | Refresh File List | F5 | |
  | N_FileList_Clear | Clear File List | Ctrl+N | |
  | — | | | |
  | N_FileList_Copy | Copy File to Computer | Shift+C | → FormCopy |
  | N_FileList_DelDisk | Delete File from Disk | Shift+Del | |
  | N_FileList_DelDiskAll | Delete All Unlocked Files from Disk | | |
  | N_FileList_Save | Save whole group | | MN-010 |
  | N_FileList_Repair | Repair broken file | | MN-011 |
  | — | | | |
  | N_FileList_SetLock | Set Lock (Protect from Deletion) | | |
  | N_FileList_UnsetLock | Unlock | | |
  | — | | | |
  | N_FileList_Group_With | Group with . . . | | |
  | N_FileList_Ungroup | Ungroup | | |
  | N_FileList_Group_Auto | Find Groups Automatically | | |
  | N_FileList_Ungroup_All | Ungroup All | | |
  | — | | | |
  | N_FileList_Track_Google | Open Track in Google Earth | | |
  | N_FileList_Track_Export | Export Track | | |
  | N_FileList_Track_Set | Attach Track | | |
  | N_FileList_Track_GPSLog | Show Related GPS Log | | **Visible = False** in the DFM |
  | N_FileList_Track_Move | Move video position | | |
  | — | | | |
  | N_FileList_ShowExplorer | Show File in Explorer | | |
  | N_FileList_Property | Property | | → FormFileProperty |

  Enabled and visible logic lives in `PopupMenuListViewPopup` (L3519–3565; FL spec). Checked states of pop-up items are not persisted, because their root is not the main menu.

### 5.4 Menu entry points covered in detail here

- **MN-010** **Save video / export** (`N_File_Save_VideoClick L4904–4944`, `N_FileList_SaveClick L4949–4973`). Both entry points fill and show the **FormCropping** singleton without modal blocking:
  - **Save selected fragment** (Ctrl+S, also in the graphics pop-up) adds, for each sub-file, the part of it that overlaps the A–B selection. It proposes the output name `<copyFolder>\Fragment of <fileFace>[ (HH-MM-SS - HH-MM-SS)].<ext>`. Without a selection the window simply opens.
  - **Save whole group** (list pop-up) saves the whole current group if a file is loaded. Otherwise it adds every sub-file of every selected row as a whole file.

  The authoritative step-by-step rules are SF-CROPPING-035 and SF-CROPPING-036 (§6.5). The export itself is SF-CROPPING-023 to SF-CROPPING-034.
- **MN-011** **Repair** (`N_File_Repair_VideoClick L4976–4978`, `N_FileList_RepairClick L4983–4992`).
  - The File menu opens FormRepair with no file and no file manager (`TFormRepair.Open(nil, nil)`).
  - The list pop-up item acts only when a file is loaded. It passes the first sub-file whose video `durationState` is 2 (presumably "duration unreadable", i.e. broken), or sub-file 0 if there is none, together with the file manager. FormRepair is specified in §6.8.
- **MN-012** **Convert GPS** (`N_File_Convert_GPSClick L3969–3972`):
  1. Collect the GPS log files known to the file manager (`GetLogFiles`).
  2. Pop up FormGPSLog with them.
  3. Stop playback.

  The form itself (TFormGPSLog) is specified in the file-list spec.
- **MN-013** **Setup device** (`N_SetupDeviceClick L5211–5213`, §6.9), **About** (L2901–2903, §6.1) and **What's New** (L5226–5228, §6.2) only obtain the lazily created form and `Show` it non-modally. Calling again while the form is open brings it to the front.
- **MN-014** **Copy** (`N_FileList_CopyClick L4502–4512`) collects all sub-files of all **selected** rows into one item (`AddItem`/`AddSubItem` L4490–4501). If there are any, it creates a **new** FormCopy instance and calls `Popup(item, FormMain)` (§6.4). If the form did not stay visible (nothing to do or cancelled), it is closed.
- **MN-015** **Property** (`N_FileList_PropertyClick L3962–3966`): for **each** selected row, open a separate FormFileProperty (`TFormFileProperty.Popup(item)`; §6.7).
- **MN-016** **Visit site** opens the URL from translation with the system default handler (`ShellExecute` "open"), as listed in the Help table.

---

## 6. Secondary forms

### 6.0 Common rules, index and form-owned settings

**Index of forms in scope**

| § | Form (unit) | Opened from | Modality / instancing |
|---|---|---|---|
| 6.1 | TFormAbout (UnitFormAbout) | Help → About (F1) | singleton, non-modal |
| 6.2 | TFormWhatsNew (UnitFormWhatsNew) | Help → What's New?; `/whatsnew` switch | singleton, non-modal |
| 6.3 | TFormShowMessage (UnitShowMessage) | `rvShowMessage(...)` from anywhere | new instance per message; modal or modeless |
| 6.4 | TFormCopy (UnitFormCopy) | list pop-up → Copy File to Computer (Shift+C) | **new instance per call**, non-modal |
| 6.5 | TFormCropping (UnitFormCropping) | File → Save selected fragment (Ctrl+S); list pop-up → Save whole group; graphics pop-up | singleton, non-modal |
| 6.6 | TFormCroppingItem (UnitFormCroppingItem) | FormCropping → Edit | singleton, **modal** |
| 6.7 | TFormFileProperty (UnitFormFileProperty) | list pop-up → Property | **new instance per selected row**, non-modal |
| 6.8 | TFormRepair (UnitFormRepair) | File → Repair broken file; list pop-up → Repair | singleton, non-modal, top-level (no owner) |
| 6.9 | TFormSetupDevice (UnitFormSetupDevice) | File → Setup Device | singleton, non-modal |
| 6.10 | TFormUpdateExe (UnitFormUpdateExe) | background update checker | singleton, non-modal |
| 6.11 | TFormUpdateFirmware (UnitFormUpdateFirmware) | background update checker + memory-card detection | singleton, created hidden, shows itself |
| 6.12 | Network support (UnitHttpDownloader, UnitServerInfo, AddUrlAddons, UnitBrand) | — | — |

**Covered by other spec writers.** These forms are listed only for completeness; their requirements are not in this document.

| Form | Covered by |
|---|---|
| TFormGroup (UnitFormGroup) | file list (Group with …) |
| TFormSelectFolder (UnitFormSelectFolder) | file list / copy; this is the folder chooser used by SF-COPY-003 |
| TFormDetails (UnitFormDetails) | graphs/map (Details panel or window) |
| TFormGPSLog (UnitFormGPSLog) | file list (Convert GPS Log, Show related GPS log) |
| TFormFullScreen (UnitFormFullScreen) | playback/video |
| TFormSetupSubtitleTemplate (UnitFormSetupSubtitleTemplate) | playback / export (subtitle template, used by SF-CROPPING-021) |

**Shared rules for all secondary forms**

- **SF-COMMON-001** **Esc closes** any secondary form that is the active form. This is done by the global shortcut hook in SH-023 (`UnitFormMain: TFormMain.ApplicationEventsShortCut L4441–4442`).
- **SF-COMMON-002** **Re-translation.** When the language changes, `TFormMain.DoTranslate` re-applies fonts and texts to every secondary form that already exists (TR-021, TR-030). A newly created singleton is translated immediately on creation (`UnitUtils: GetFormAndCreate L237–244`).
- **SF-COMMON-003** **Taskbar and ownership.** A form passed to `SetFormParent(form, nil)` gets its own taskbar button. A form given a parent is owned by that parent and is centred over it the first time it is shown (`UnitUtils: SetFormParent L211–234`). In Rust, secondary windows become separate viewports or egui windows, centred over the main window on first show.

**Conventions used in 6.4 to 6.7**

- "Translate(K)" = UI string looked up by translation key K (English text from TRANSLATE_EN.TXT shown in quotes). Keys missing from TRANSLATE_EN.TXT (e.g. `points`, `GPS Log`) fall back to the key text itself (see section 4).
- `rvShowMessage(text, caption, modal)` = the app's own message window (TFormShowMessage); third argument is a modal flag (1 = modal, 0 = modeless) — UnitShowMessage: rvShowMessage L127–L135.
- `MessageDlg(..., mtConfirmation, [Yes, No])` = standard Yes/No confirmation box.
- `filterFileName` replaces each of `" * < > ? |` and any `:` that is not the 2nd character with `_` (UnitUtilsFile: filterFileName L310–L314).
- `SetFormParent(form, nil)` gives the window its own taskbar button (WS_EX_APPWINDOW), no owner, and, if not yet visible and Position is not poScreenCenter/poDesigned-like, centres it over the main window; with a parent it becomes owned by that parent and is centred over it (UnitUtils: SetFormParent L211–L234). `GetFormAndCreate` creates a singleton form on first use and applies SetFormParent (UnitUtils: GetFormAndCreate L237–L241).
- Time values "in days" are Delphi TDateTime units (1 day = 1.0).
- TimeDurationToStrD(withMs=false, ".") renders a duration as `H:MM:SS` (no milliseconds).

**Conventions used in 6.1 to 6.3 and 6.8 to 6.12**

Common facts for every form in this section:

- **Lazy singletons.** Each form has a global accessor (`FormAbout()`, `FormRepair()`, ...). On first call it creates the form, sets its owner/parent window and immediately runs `TFormMain.DoTranslate` so captions are localised; later calls return the same instance (UnitUtils: GetFormAndCreate L237–244). Parent is the main form for every form here except **TFormRepair**, which has no parent and is a top-level window (UnitFormRepair: FormRepair L59–61).
- **Captions/labels are set by the main form's DoTranslate** (UnitFormMain: TFormMain.DoTranslate L4714–4844), not by the form itself. Only forms that already exist are re-translated when the language changes.
- **Branding.** This build is OEM-branded "DATAKAM PLAYER". The literal string `DATAKAM PLAYER` is hard-coded in About, WhatsNew, ShowMessage, the HTTP User-Agent, and the mail subject. Contact/site strings come from the translation tables (`MyEMail`, `MySite`, `MySiteBrand`, `MyCity`, `MyName`). TRANSLATE_FIX_ALL.TXT (brand override) sets them to `help@datakam.com`, `http://datakam.com`, `DATAKAM technology Co.Ltd., Taiwan` and `+86 (755) 2227-1965`. TRANSLATE_ALL.TXT holds the original author's values (`vadikus@mail.ru`, `http://www.registratorviewer.com/index_en.html`). UnitBrand.c holds only unit init/finalisation stubs (UnitBrand: initialization L15) and has no runtime logic.
- `Translate(key, backupKey)` returns the key text itself when neither key is found (UnitTranslate: Translate L83–88). Several forms rely on this to detect a missing key.
- **Error-dialog convention.** The window caption is usually `<form caption>. <Translate("Error")>`. The message box is always `rvShowMessage(text, caption, modal=true)` (see 6.3).

**Persistent settings owned by secondary forms.** These are also listed in the table in §3.2.

| Value | Type | Default | Used by |
|---|---|---|---|
| `copyFolder` | string | My Videos / My Documents; `C:\` when read with an explicit default | Read by FormCopy (SF-COPY-003), Save fragment (SF-CROPPING-035) and FormRepair (SF-REPAIR-008). Written by FormCopy when a folder is chosen, by FormCropping with the output folder at Start (SF-CROPPING-023), and by FormRepair when a result file is chosen (SF-REPAIR-007). |
| `id`, `gps` | string | — | Sent by the update checker (SF-NET-003). Card detection adds entries to `gps` (§6.11). |

No other secondary form in scope reads or writes the registry. The following are session-only and are **not persisted**: `descardUpdate` (the update was declined this session) and the server globals (§6.12). FormSetupDevice writes the file `X:\setup.cfg` on the memory card (§6.9), not the registry.

---

### 6.1 TFormAbout (UnitFormAbout)

- **Purpose:** Fixed-size "About" box with logo, product name, version, and the brand's contact details. Mail and site are clickable.
- **Opened from:** main menu `N_Help_About` → non-modal `Show` (UnitFormMain: TFormMain.N_Help_AboutClick L2901–2903).
- **Inputs/outputs:** reads the global `Self_Version_Full` (e.g. `6.0.0.22`, built from the EXE version resource: UnitUtils L530–542), the translation keys below, and the main-menu state `N_Help_Site_Brand.Visible`. Returns nothing.

**Layout** (DFM): `BorderStyle=bsSingle`, `BorderIcons=[biSystemMenu]`, `BorderWidth=10`, `AutoSize=True`, design client area 300×322, `Color=clWhite`, which is replaced at runtime by `colorAboutBG`. Default font MS Sans Serif 8pt. Caption = `N_Help_About` ("About").

| Control | Class | L,T,W,H | Properties / caption source |
|---|---|---|---|
| ImageLogo | TImage | 0,0,300,158 | AutoSize. Bitmap resource `Form_About_Logo.bmp`. |
| LabelTitle | TLabel | 16,168,204,40 | Bold, height −29. The DFM font is Comic Sans MS; runtime forces **Arial**. Text = `DATAKAM PLAYER`. |
| LabelVer | TLabel | 128,207,26,13 | Bold 8pt. Text = `v` + Self_Version_Full. |
| LabelName | TLabel | 24,246 | taCenter, transparent. Text = `MyName` (EN: "Vadim Andreevich Kozlov"; brand: phone number). |
| LabelMail | TLabel | 24,265 | Hand cursor, hint = `SendMail` ("Send e-mail"), black 8pt. Text = `MyEMail`. |
| LabelCity | TLabel | 24,284 | taCenter. Text = `MyCity`. |
| LabelSite | TLabel | 41,306,102,16 | Hand cursor, Verdana height −13, hint = `N_Help_Site` ("Visit Site"). Text = `MySiteBrand`, falling back to `MySite`. |

**Requirements**

- **SF-ABOUT-001** On creation, the form background colour is the translation colour entry `colorAboutBG` (EN `#FFFFFF`). The window icon is `icon_about.ico` and the title font is Arial. (UnitFormAbout: TFormAbout.FormCreate L48–52)
- **SF-ABOUT-002** On creation, the vertical positions of LabelName and LabelCity are swapped. Final order top→bottom is therefore City/company, Mail, Name/phone. (UnitFormAbout: TFormAbout.SwapLabels L71–75, called from FormCreate L53)
- **SF-ABOUT-003** On creation, the logo bitmap is loaded and moved up so its bottom edge sits 5 px above the top of LabelTitle. (UnitFormAbout: TFormAbout.FormCreate L64–65)
- **SF-ABOUT-004** Each time the form is shown, LabelTitle is set to `DATAKAM PLAYER` and LabelVer to `v<Self_Version_Full>`. Title, version, name, mail, city, site and logo are each centred horizontally: left = (clientWidth − width) / 2, using the nested helper `C`. (UnitFormAbout: TFormAbout.FormShow L82–91; C L79–81)
- **SF-ABOUT-005** Hover feedback on LabelMail and LabelSite: on mouse-enter the label font turns blue (`$FF0000` in BGR order = pure blue); on mouse-leave it returns to black. (UnitFormAbout: TFormAbout.LabelMailMouseEnter L95–98, LabelMailMouseLeave L101–104)
- **SF-ABOUT-006** Clicking LabelMail opens the system mail handler with `mailto:<MyEMail>?subject=DATAKAM PLAYER`, via ShellExecute with the default verb. (UnitFormAbout: TFormAbout.LabelMailClick L107–109)
- **SF-ABOUT-007** Clicking LabelSite opens the brand site (`MySiteBrand`) if the main-menu item `N_Help_Site_Brand` is visible, otherwise the author site (`MySite`). Either way it is opened with ShellExecute (show-normal) as a URL. (UnitFormAbout: TFormAbout.LabelSiteClick L112–117; UnitFormMain: N_Help_Site_MyClick L5395–5397, N_Help_Site_BrandClick L5400–5402)
- **SF-ABOUT-008** Text assignment is done by the main form's translator: caption = `N_Help_About`; LabelMail hint = `SendMail` and text = `MyEMail`; LabelSite hint = `N_Help_Site` and text = `MySiteBrand` (backup key `MySite`); LabelCity = `MyCity`; LabelName = `MyName`. (UnitFormMain: TFormMain.DoTranslate L4832–4840)
- **SF-ABOUT-009** (trivial) On destroy, the logo bitmap is released. (UnitFormAbout: TFormAbout.FormDestroy L120–122)

---

### 6.2 TFormWhatsNew (UnitFormWhatsNew)

- **Purpose:** Resizable read-only viewer for the release notes.
- **Opened from:**
  - Main menu `N_Help_WhatsNew` → non-modal `Show` (UnitFormMain: TFormMain.N_Help_WhatsNewClick L5226–5228).
  - Automatically at startup when command-line argument 1 is exactly `/whatsnew` (UnitFormMain: TFormMain.TimerOnStartTimer L5188–5189). The self-updater passes this argument (SF-UPDATEEXE-008).
- **Inputs:** embedded RCDATA text resource chosen by the current language.

**Layout:** sizeable window 700×500 (`Width/Height`), font Lucida Console 8pt (monospace). Caption = `N_Help_WhatsNew` + ` - DATAKAM PLAYER ` + version (UnitFormMain: DoTranslate L4842–4844).

| Control | Class | Geometry | Properties |
|---|---|---|---|
| Memo | TMemo | Align=alClient (0,0,684,462) | ReadOnly, vertical scrollbar only |

**WHATSNEW file format** (resources `WHATSNEW.TXT`, `WHATSNEW_RU.TXT`, `WHATSNEW_UA.TXT`, `WHATSNEW_BY.TXT`): UTF-8 with BOM, free text. By convention each release has a 3-line banner (`=================` / `vX.Y | YYYY.MM.DD` / `=================`) followed by ` + item` lines, which may be indented for sub-items. It is displayed verbatim.

**Requirements**

- **SF-WHATSNEW-001** On creation the window icon is `icon_whatsNew.ico`. (UnitFormWhatsNew: TFormWhatsNew.FormCreate L37–39)
- **SF-WHATSNEW-002** On every show, the text resource `WhatsNew_<langid>.txt` is loaded, where `<langid>` is the current language's `langid` translation value (e.g. `ru`, `ua`, `by`). If that resource does not exist, `WhatsNew.txt` (English) is used instead. (UnitFormWhatsNew: TFormWhatsNew.FormShow L44–47)
- **SF-WHATSNEW-003** The first 3 bytes (the UTF-8 BOM) are always dropped and the remaining text is decoded from UTF-8. (UnitFormWhatsNew: TFormWhatsNew.FormShow L48–49)
- **SF-WHATSNEW-004** The memo shows the header line `DATAKAM PLAYER <Self_Version_Full>`, then a blank line, then the decoded notes. (UnitFormWhatsNew: TFormWhatsNew.FormShow L49)

---

### 6.3 TFormShowMessage (UnitShowMessage) — the app-wide message box

- **Purpose:** Replaces the VCL ShowMessage everywhere in the app. It is an auto-sized dialog with a word-wrapped text, an OK button, and a small "copy all to clipboard" button that adds diagnostic info to the copied text.
- **API:** `rvShowMessage(text, caption, showModal, textSize = 0)`. All callers in this section pass `showModal = true`. (UnitShowMessage: rvShowMessage L127–135)
  - It may be called from any thread. Form creation is marshalled to the main thread (UnitShowMessage: TFormCreater.createForm L113–122).
  - `showModal = true` → modal; the call returns after the dialog closes.
  - `showModal = false` → modeless; the call returns immediately.
  - There is **no** "don't show again" option, no registry value, and no Yes/No variant. Yes/No questions elsewhere use the standard `MessageDlg`.
- **Inputs:** text, caption, optional font size.

**Layout (DFM):** `BorderStyle=bsDialog`, `BorderIcons=[]` (no system buttons), `BorderWidth=16`, design client 174×100, MS Sans Serif 8pt. The form auto-sizes at runtime.

| Control | Class | L,T,W,H | Properties |
|---|---|---|---|
| LabelText | TLabel | 0,0 | message text |
| ButtonOk | TButton | 0,64,75,30 | Caption "Ok"; Default and Cancel (Enter/Esc close) |
| ButtonCopy | TSpeedButton | 112,64,22,22 | Flat, hand cursor, toggle (GroupIndex 1, AllowAllUp). Glyph = bitmap of main-menu item `N_FileList_Copy`. Hint = `Property_CopyAll` ("Copy All to Clipboard"). |

**Requirements**

- **SF-SHOWMESSAGE-001** If the caption is empty it becomes `DATAKAM PLAYER`. The window caption is set as Unicode: it is converted from the current language's code page to UTF-8 and then to wide text. (UnitShowMessage: TFormCreater.CreateFormSync L73–80)
- **SF-SHOWMESSAGE-002** The dialog is owned by the form that is active at call time (falls back to the screen). It uses the application UI font (`SetupFont`). (UnitShowMessage: TFormCreater.CreateFormSync L76–79, L100)
- **SF-SHOWMESSAGE-003** If `textSize > 0`, the label font size is set to it. (UnitShowMessage: TFormCreater.CreateFormSync L81–83)
- **SF-SHOWMESSAGE-004** Label width rules:
  - natural single-line width < 150 px → fixed width of 150 px;
  - natural width > 800 px → word-wrap on, width 800 px, height auto;
  - otherwise the natural width is used.

  (UnitShowMessage: TFormCreater.CreateFormSync L85–92)
- **SF-SHOWMESSAGE-005** Positioning:
  - The OK button is placed 2×BorderWidth (32 px) below the label's bottom edge.
  - The copy button's bottom is aligned with the OK button's bottom.
  - The form then auto-sizes to its content.
  - On every resize, OK is centred horizontally and the copy button is right-aligned.

  (UnitShowMessage: CreateFormSync L94–98; TFormShowMessage.FormResize L144–147)
- **SF-SHOWMESSAGE-006** Modal mode shows the dialog modally and frees it afterwards. Modeless mode shows it, brings it to front and repaints. Closing the form always frees it. (UnitShowMessage: CreateFormSync L101–106; FormClose L139–141)
- **SF-SHOWMESSAGE-007** OK closes the dialog. Key-up of **Space** also acts as OK. (UnitShowMessage: ButtonOkClick L150–152; FormKeyUp L172–174)
- **SF-SHOWMESSAGE-008** **Ctrl+C** gives visual feedback and then copies: the copy button is shown pressed, the window repaints, the app waits 500 ms, then performs the copy. (UnitShowMessage: TFormShowMessage.FormKeyUp L175–179)
- **SF-SHOWMESSAGE-009** The copy button puts this text on the clipboard:
  1. A diagnostic header line: `DATAKAM PLAYER <Self_Version_Full>, Win<major>.<minor>, <current date-time>` plus CRLF. The Windows version comes from GetVersionEx. The exact separators are inferred from the string pool.
  2. A line `-`.
  3. Only if the window caption is not `DATAKAM PLAYER`: the caption, then a line `-`.
  4. The message text.

  The text is copied in the current language's code page, and the button is released afterwards. (UnitShowMessage: TFormShowMessage.ButtonCopyClick L158–168)

---

### 6.4 TFormCopy (UnitFormCopy) — "Copy File to Computer"

**Purpose.** Copies all files belonging to the selected file-list entries (video, its meta/info file, its separate GPS log) into a user-chosen folder, with progress, speed, remaining time, pause and "open folder".
**Opened from.** File-list popup `N_FileList_Copy` "Copy File to Computer" (Shift+C) → `TFormMain.N_FileList_CopyClick` (UnitFormMain L4502–L4510): gathers all sub-items of all selected entries into one combined item (helpers AddItem/AddSubItem L4490–L4501); if there is at least one, creates a **new** TFormCopy instance (not a singleton) and calls Popup; if the form did not become visible (folder dialog cancelled) it is closed.
**Inputs.** Combined file item (sub-items), parent = main form. **Outputs.** Copied files; registry `copyFolder`.

**Layout** (DFM: Width 461, Height 207, BorderIcons system menu + minimise, MinWidth 350, Position poDefaultPosOnly, no scrollbars; caption Translate(`Copying_Title`) "Copying" + " " + display name of the first item; icon `icon_copy.ico`; double-buffered; own taskbar button). At create time the client height is fixed to exactly fit the labels + progress bar and both Min and Max height constraints are set to that height (only width resizable).

| Control | Class | L/T/W/H | Caption (key → English) | Notes |
|---|---|---|---|---|
| LabelFileCaption | TLabel | 24/6/23/13 | `File`+":" "File:" | right-aligned 3 px left of LabelFile |
| LabelFile | TLabel | 56/6/16/13 | "i / n  -  <source path>"; at end: last destination path | |
| ButtonOpenFolder | TSpeedButton | 184/1/89/21 | `OpenFolder` "Open Folder" | flat-looking, underlined font; right-aligned (ClientWidth − Width − 1) |
| LabelSizeCaption | TLabel | 16/24/27/13 | `Size`+":" "Size:" | |
| LabelSize | TLabel | 56/24/20/13 | "P%  -  C Mb / T Mb"; at end "C Mb" | Mb = Translate(`Mb`) |
| LabelSpeedCaption | TLabel | 184/24/38/13 | `Speed`+":" "Speed:" | hidden at end |
| LabelSpeed | TLabel | 232/24/31/13 → ClientWidth−100 | "X.X Mbyte/sec" (`Mbytepsec`) | hidden at end |
| LabelTimeCaption | TLabel | 16/42/30/13 | `Time`+":" "Time:" | |
| LabelTime | TLabel | 56/42/23/13 | elapsed H:MM:SS (excluding pauses) | |
| ButtonPause | TSpeedButton | 112/39/41/19 | `Pause` "Pause" | toggle (GroupIndex 1, AllowAllUp behaviour), underlined; hidden at end |
| LabelRemainCaption | TLabel | 176/42/48/13 | `Remains`+":" "Remains:" | hidden at end |
| LabelRemain | TLabel | 232/42/41/13 → same left as LabelSpeed | remaining H:MM:SS | hidden at end |
| ProgressBar | TProgressBar | 8/63/273/17 | 0..100 | width = ClientWidth − 2×8; bottom-aligned with 8 px margin |
| Timer | TTimer | — | — | interval 100 ms, initially disabled |

**Requirements**

- **SF-COPY-001 (create).** Load icon, double buffering, apply UI font (also to the two link buttons), own taskbar button, fix height as described, set caption and all label/button captions from translation keys. — TFormCopy.FormCreate L229–L247.
- **SF-COPY-002 (resize).** Re-position as in the layout table: Open Folder right-aligned, speed/remain values at ClientWidth−100, every caption label right-aligned 3 px left of its value, progress bar stretched and pinned to bottom. — FormResize L253–L264.
- **SF-COPY-003 (start).** Popup: read registry `copyFolder` (default "C:\"); show the shared Select-Folder dialog (TFormSelectFolder, owned by the main window) pre-filled with it and captioned Translate(`SelectFolderFor`) "Select Folder for" + " " + display name. On OK: read the folder, store it back to `copyFolder`, start the copy thread, record start time, update the labels once, append " " + display name to this window's caption, show the window (modeless) and start the 100 ms timer. On cancel: close (and free) the form. — Popup L269–L284.
- **SF-COPY-004 (file list).** The worker collects, for every sub-item, up to three files: the video file, the info/meta file and the GPS-log file (skipping empty names); each file's size is taken from the item or, if unknown, from disk; totals are summed. Destination folder gets a trailing "\" and is created recursively; a 1 MiB copy buffer is allocated. — TFormCopyThread.Create L73–L93; AddFile L99–L107.
- **SF-COPY-005 (copy loop).** For each file in order: destination = folder + source file name. If a destination exists: if it is "the same" (equal size and equal last-write time) skip it silently; otherwise ask on the UI thread "File already exists⏎<dest>⏎⏎Do you want to replace it ?" (Yes/No, keys `Mes_FileExists`, `Mes_SureToReplace`); No → skip. Stop when the form is closed (thread terminated). — Execute L113–L134; QueryExistingFile L207–L210; SyncQueryExistingFile L213–L223; UnitUtilsFile: FileIsSame L558–L563.
- **SF-COPY-006 (single file copy).** Open source read-only/shared; create destination if missing, otherwise clear its attributes (removes read-only); copy source timestamps to it; open destination for writing; if destination is larger than source truncate to 0, then pre-size it to the source size; copy in 1 MiB blocks from offset 0 (no resume) until done/terminated/zero read, accumulating copied bytes. Throughput: every > 4 s compute bytes/s over the interval; first sample sets the speed; afterwards new = 0.5·old + 0.5·sample when the sample is ≥ old, else 0.2·old + 0.8·sample. While paused, sleep in 10 ms steps. — DoCopy L150–L195.
- **SF-COPY-007 (after copy).** When all files are processed set current index = count, wait 0.5 s, then copy original timestamps onto every destination file again. — Execute L135–L141.
- **SF-COPY-008 (progress timer, every 100 ms).** While copying: LabelFile = "<i> / <n>  -  <source path>"; percent = clamp(round(copied × 100 / max(total,1)), 0, 100); LabelSize = "<percent>%  -  <copied MiB> Mb / <total MiB> Mb" (integer MiB); progress bar = percent; if speed > 0: LabelSpeed = "<MiB/s, 1 decimal> Mbyte/sec", LabelRemain = max(total − copied, 0) / speed as H:MM:SS; else both blank. When finished: stop timer, LabelFile = last destination path, LabelSize = "<copied MiB> Mb", bar full, hide speed/remain captions+values and the Pause button, beep. Unless paused, LabelTime = now − start (pause time excluded). — TimerTimer L301–L333.
- **SF-COPY-009 (pause).** Pause toggles the worker's pause flag; on pause remember the moment; on resume shift the start time forward by the paused interval. — ButtonPauseClick L354–L360.
- **SF-COPY-010 (open folder).** If not already paused, pause; open Explorer with the current destination file selected; if it was not paused before, wait 3 s and resume. (The 3 s wait blocks the UI.) — ButtonOpenFolderClick L339–L351.
- **SF-COPY-011 (close/cancel).** Closing the window (no Cancel button — the system close box is the cancel) stops the timer, terminates and frees the worker (partially written file remains) and frees the form. — FormClose L289–L296.
- **SF-COPY-012 (unit init/final).** Reference counter only. — UnitFormCopy: initialization L362.

---

### 6.5 TFormCropping (UnitFormCropping) — "Cutting" / export of fragments ("Save selected fragment")

**Purpose.** Batch editor + exporter that concatenates one or more fragments of dash-cam video files (each with its own start/end fraction and playback speed-up) into a single MP4/MOV, optionally embedding the GPS/G-sensor track ("meta info") and writing a companion `.srt` subtitle file. No re-encoding: video/audio packets are copied (speed-up is done by FPS change or key-frame-only, see 6.6).

**Opened from.**
- Main menu File → `N_File_Save_Video` "Save selected fragment" (Ctrl+S) → `TFormMain.N_File_Save_VideoClick` (UnitFormMain L4904–L4944): adds the current A–B selection of every sub-file of the current (grouped) item.
- File-list popup `N_FileList_Save` "Save whole group" → `TFormMain.N_FileList_SaveClick` (UnitFormMain L4949–L4973).
- Singleton created lazily via `FormCropping()` (UnitFormCropping L115–L117) with no parent (own taskbar button, centred on main window). The main form's `DoTranslate` re-translates it if it exists (UnitFormMain L4616–L4617, L4781–L4796).
- TFormRepair borrows this form's `SaveDialog` for its own "result file" picker (UnitFormRepair: TFormRepair.ButtonFileResultClick L307–L312) — out of scope here, but the SaveDialog must be shareable.

**Inputs.** List of crop items (`TCropItem`, one per source file): videoFileName, duration (ms, from container), p1/p2 (start/end as fraction 0..1 of file duration), sysTime (wall-clock start of file), speed (1 = normal, >1 speed-up, <1 = 1/N), speedUpFPS (true = keep all frames/raise FPS, false = key frames only), fps (default 30), mi (optional moving-info = GPS track object), mov (opened container), gopInfo (GOP pattern string), frameSize.
**Outputs.** Output video file (user-chosen path), optional `<same name>.srt`, file timestamps set, output added to main file list; registry `copyFolder` updated.

**Layout** (DFM: Width 600, Height 429, BorderWidth 10, sizeable, no scrollbars, caption key `Cropping_Title` "Cutting", icon resource `icon_cropping.ico`, accepts Explorer file drops).

| Control | Class | Parent | L/T/W/H | Align | Caption / text (key → English) | Notes |
|---|---|---|---|---|---|---|
| PanelCenter | TPanel | form | 0/0/564/179 | alClient | — | bevel none |
| ListView | TListView | PanelCenter | 0/0/539/169 | alClient | columns: `File` "File" (autosize); `Started`+" - "+`Finished` "Started - Finished" (110, right); `Speed` "Speed" (70, right); `Duration` "Duration" (65, right) | report view, grid lines, multi-select, row select, read-only, virtual (OwnerData), HideSelection=false, popup = PopupMenuCropList, dbl-click = N_Edit |
| PanelRight | TPanel | PanelCenter | 539/0/25/169 | alRight, AutoSize | — | holds 25×25 speed buttons |
| ButtonAdd | TSpeedButton | PanelRight | 0/0/25/25 | | glyph of N_FileList_Add, hint "Add Files" | |
| ButtonDel | TSpeedButton | PanelRight | 0/25/25/25 | | glyph/hint of N_FileList_Del "Remove from List" | |
| ButtonClear | TSpeedButton | PanelRight | 0/50/25/25 | | glyph/hint of N_FileList_Clear "Clear File List" | |
| ButtonUp | TSpeedButton | PanelRight | 0/82/25/25 | | glyph/hint of N_Up "Up" | |
| ButtonDown | TSpeedButton | PanelRight | 0/107/25/25 | | glyph/hint of N_Down "Down" | |
| ButtonEdit | TSpeedButton | PanelRight | 0/140/25/25 | | glyph/hint of N_Edit "Edit" | |
| PanelSpace | TPanel | PanelCenter | 0/169/564/10 | alBottom | — | spacer |
| PanelBottom | TPanel | form | 0/179/564/192 | alBottom, AutoSize | — | |
| LabelTimePos | TStaticText | PanelBottom | 0/0/67/17 | | idle: "(N) H:MM:SS"; running: "P% - H:MM:SS" | |
| LabelCurrentAction | TStaticText | PanelBottom | 121/0/94/17 | | current stage text, empty when idle | |
| GroupBoxVideoFileTracks | TGroupBox | PanelBottom | 0/23/161/89 | | `Cropping_FileTracks` "Video File Tracks" | width = half of panel at runtime |
| CheckBoxVideo | TCheckBox | GroupBoxVideoFileTracks | 8/18/97/17 | | `Video` "Video" | checked |
| CheckBoxAudio | TCheckBox | GroupBoxVideoFileTracks | 8/40/96/17 | | `Audio` "Audio" | checked |
| CheckBoxMeta | TCheckBox | GroupBoxVideoFileTracks | 8/62/111/17 | | `MetaInfo` + " (GPS, G-Sensor)" → "Meta Info (GPS, G-Sensor)" | checked |
| GroupBoxAdditionalFiles | TGroupBox | PanelBottom | 185/23/249/50 | | `Cropping_AddonFiles` "Additional Files" | |
| CheckBoxSRTSubtitles | TCheckBox | GroupBoxAdditionalFiles | 8/16/185/17 | | "(.srt) - " + `Subtitles` → "(.srt) - Subtitles" | unchecked |
| ButtonSubtitle | TButton | PanelBottom | 185/86/145/25 | | `SetupSubtitle_Title` "Setup Template for Subtitles" | width = min(300, groupbox width) |
| EditFileName | TEdit | PanelBottom | 0/129/409/21 | | output file path | |
| SpeedButtonSelectFile | TSpeedButton | PanelBottom | 412/128/21/21 | | "..." | |
| ButtonStart | TButton | PanelBottom | 169/161/97/31 | | `Cropping_BtnStart` "Start" | centred with Cancel |
| ButtonCancel | TButton | PanelBottom | 289/161/97/31 | | `Cancel` "Cancel" | |
| SaveDialog | TSaveDialog | — | — | | filter "MP4 and MOV files\|*.mp4;*.mov\|Any Files\|*.*", DefaultExt "mp4", overwrite prompt | |
| PopupMenuCropList | TPopupMenu | — | — | | items below (captions translated by item name, AutoHotkeys manual) | |

Popup menu items (top→bottom): `N_Edit` "Edit" (Enter), `N_Speed` caption = Translate(`Speed`) "Speed" (no shortcut; DoTranslate sets the caption), `N_NoIntersection` "Remove Intersections by Time", separator, `N_Up` "Up" (Alt+Up), `N_Down` "Down" (Alt+Down), separator, `N_FileList_Add` "Add Files" (Insert), `N_FileList_Del` "Remove from List" (Delete), `N_FileList_Clear` "Clear File List" (Ctrl+N). Each item's bitmap is loaded from resource `<ItemName>.bmp` (e.g. N_EDIT.BMP, N_UP.BMP, N_DOWN.BMP, N_FILELIST_ADD.BMP, N_FILELIST_DEL.BMP, N_FILELIST_CLEAR.BMP) with transparency; N_Speed and N_NoIntersection reuse N_Edit's bitmap.

**Requirements**

- **SF-CROPPING-001 (create).** On creation: load the window icon, register the window as a drop target for files, load each popup item's bitmap from resource `<name>.bmp` (transparent), give N_Speed and N_NoIntersection the N_Edit bitmap, then run the close/reset routine once (SF-CROPPING-004) so the form starts empty. — UnitFormCropping: TFormCropping.FormCreate L127–L136.
- **SF-CROPPING-002 (side buttons).** On show, each side button (Add, Del, Clear, Up, Down, Edit) is bound to the corresponding popup item: caption cleared, glyph = item bitmap, click = item's handler, tooltip = item caption (shown), and releasing the mouse on the button returns keyboard focus to the list if it can take focus. — UnitFormCropping: Btn L141–L148; FormShow L150–L155; ButtonAddMouseUp L324–L327.
- **SF-CROPPING-003 (show).** On show: reset progress display (DrawProgress(0,0)), recompute totals (SF-CROPPING-012) and clear the current-action label. — FormShow L156–L158.
- **SF-CROPPING-004 (close/reset).** On close (and once on create): if the Start button is enabled (i.e. not currently processing) clear the job list and the output file name; always reset progress, set "current row" to none (−1), progress to 0, recompute totals and re-enable all inputs (SetEnables(true)). The form is only hidden (singleton is kept). — FormClose L162–L172.
- **SF-CROPPING-005 (layout on resize).** Start and Cancel are centred horizontally in the bottom panel, separated by BorderWidth (10 px); the "..." button is right-aligned and the file edit fills the width to its left; the "Video File Tracks" group takes half of the panel width, "Additional Files" starts 16 px to its right and fills the rest; check-boxes fill their groups (minus left margin ×2); the subtitle-template button is left-aligned with "Additional Files" and is min(300, that group's width) wide; the list width is nudged by 1 px to force column auto-size. — FormResize L175–L190.
- **SF-CROPPING-006 (enable/disable).** SetEnables(b) sets enabled state of: file edit, Start, the four check-boxes, "...", all six side buttons, subtitle-template button; when disabled the list loses its popup menu and its double-click action, when enabled both are restored (double-click = Edit). Cancel is never disabled. — SetEnables L193–L215.
- **SF-CROPPING-007 (choose output file).** "..." opens the save dialog pre-filled with the sanitised current file name (filterFileName); on OK the chosen path replaces the edit text. — SpeedButtonSelectFileClick L218–L222.
- **SF-CROPPING-008 (add files).** "Add Files" (Insert): clear the list selection, open the main window's multi-select video open dialog (`OpenDialogFileAdder`, filter "DashCam Files (*.mov, *.mp4)|…|Video Files|…|Any Files|*.*"); each chosen file is added as a whole-file item (p1 = 0, p2 = 1, no GPS object, start time derived from file); if the list was empty before, clear the selection again afterwards; nudge list width to refresh. — N_FileList_AddClick L227–L236.
- **SF-CROPPING-009 (drop files).** Files dropped from Explorer are added the same way (paths ≥ 255 chars are ignored), after clearing the selection; afterwards focus the list. — WMDropFiles L334–L352.
- **SF-CROPPING-010 (AddFile — validation and item construction).** For each file: open it as a source container. If it cannot be opened, has no video track, or has zero duration → modeless message: text = `<file>` + blank line + Translate(`Cropping_FormatUnknown`) "File has unknown format", caption = `<form caption>. <Add Files caption>`; clear the output file-name edit; the file is not added. Otherwise append an item with: file name, p1/p2 as given, duration (ms) from the container, start wall-clock time = given value if > 1.0 (a real date), else file last-write time minus duration; fps, GOP pattern string and frame size from the video track (fps 30 and empty GOP if no track info); speed = 1; speedUpFPS = true; GPS object = clone of the given moving-info if any. — AddFile L362–L392, L394–L395, L365–L367.
- **SF-CROPPING-011 (AddFile — frame-size check).** After appending, compare the new item's frame size with every earlier item; on the first mismatch show a modeless message listing `<earlier file>` + its `WxH`, the new file + its `WxH`, and Translate(`Cropping_FrameNotEqual`) "Frame size not match. Files only with same formats can be joined.", caption as in SF-CROPPING-010, and clear the output file-name edit. The item is **kept** in the list (warning only). Then update the virtual list count, select and scroll to the new row, recompute totals, and pump the message loop. — AddFile L396–L415.
- **SF-CROPPING-012 (totals).** Total output duration = Σ over items of duration × (p2 − p1) / speed. LabelTimePos shows `(<item count>) <H:MM:SS>`; Start is enabled only when the list is non-empty; list is refreshed. — CalcTotal L455–L465.
- **SF-CROPPING-013 (list row rendering).** Virtual list row i shows: col 1 file name (full path); col 2 `<start> - <end>` where each is the time-of-day (H:MM:SS) of sysTime + p × duration + user time-zone shift; col 3 speed: `1/N x`-style text for speed < 1 rendered as "1/" + round(1/speed) + "x", "1x" for speed = 1, and for speed > 1 the prefix "(fps) " (speedUpFPS) or "(key) " (key frames) + round(speed) + "x"; col 4: for the row currently being processed `<round(progress×100)>%`, otherwise the output duration of that item (duration × (p2 − p1)/speed) as H:MM:SS. — ListViewData L422–L447.
- **SF-CROPPING-014 (remove).** "Remove from List" (Delete): for all selected rows (from last to first) deselect, release the item's GPS object and container, and remove it; update count; recompute totals. — N_FileList_DelClick L242–L255; ClearItem L258–L266.
- **SF-CROPPING-015 (clear).** "Clear File List" (Ctrl+N): empty the list, releasing every item's GPS object and container; recompute totals. — N_FileList_ClearClick L271–L277.
- **SF-CROPPING-016 (move up/down).** Up (Alt+Up): scanning from row 1 downward, every selected row whose predecessor is not selected is swapped with it (data and selection state). Down (Alt+Down): scanning from second-last upward, every selected row whose successor is not selected is swapped with it. — N_UpClick L295–L301; N_DownClick L306–L312; Swap L283–L289.
- **SF-CROPPING-017 (edit item).** Edit (Enter / double-click / ButtonEdit): if a row is selected, open TFormCroppingItem modally for the first selected item (6.6), then repaint the list and recompute totals. — N_EditClick L315–L321.
- **SF-CROPPING-018 (quick speed).** "Speed" popup item: for the focused item index, pre-fill an input box (caption Translate(`N_Edit`) "Edit", prompt Translate(`Speed`) "Speed") with "N" (speed ≥ 1, rounded) or "1/N" (speed < 1). On OK: text starting with "1/" → speed = 1/max(1, int(rest, default 1)); otherwise speed = max(1, int(text, default 1)). Apply to **all selected** items and set their method to key-frames-only (speedUpFPS = false). Recompute totals. — N_SpeedClick L869–L888.
- **SF-CROPPING-019 (remove intersections).** "Remove Intersections by Time": walking from the second-last item backwards, if the next item's absolute start time is earlier than this item's absolute end time, move this item's end (p2) back to the next item's start (fraction computed with duration ≥ 1 ms); if that makes p2 ≤ p1 the item is removed entirely. Recompute totals. — N_NoIntersectionClick L895–L913.
- **SF-CROPPING-020 (popup enabling).** On popup: Edit, Speed, Up, Down and Remove are enabled only if at least one row is selected (Add, Clear, Remove Intersections always enabled). — PopupMenuCropListPopup L919–L931.
- **SF-CROPPING-021 (subtitle template).** "Setup Template for Subtitles" opens the (modeless) TFormSetupSubtitleTemplate owned by this form (covered by the subtitle-template spec). — ButtonSubtitleClick L934–L936.
- **SF-CROPPING-022 (Cancel).** Cancel hides then closes the form. If processing is running, hiding the form is the cancellation signal: every stage checks form visibility and the copy-progress callback returns "visible", so processing stops at the next check (SF-CROPPING-030). — ButtonCancelClick L469–L472; NewStage L633–L647; NewItem L650–L657; DrawProgress L517.
- **SF-CROPPING-023 (Start — validation).** On Start: (a) empty output name → modal message text Translate(`Repair_FullOutputFile`) "Result", caption Translate(`Error`) "Error", focus the edit, stop. (b) Store the output folder into registry `copyFolder`. (c) If the output file exists → confirmation "File already exists⏎<path>⏎⏎Do you want to replace it ?" (keys `Mes_FileExists`, `Mes_SureToReplace`; Yes/No); No → stop. (d) Create the output folder (recursive); if it still does not exist → modal message "Directory not exists⏎<folder>" (key `Mes_DirectoryNotExists`), caption "Error", stop. — ButtonStartClick L659–L681.
- **SF-CROPPING-024 (Start — preparation).** Stop main-window playback if playing (Play button up + play-click), disable inputs (SetEnables(false)), focus the list, reset the track buffer, create the subtitle text buffer, set tMin = tMax = now, **hide the main window** for the duration, compute currentFullDurationSec = Σ duration × (p2 − p1)/speed / 1000. — ButtonStartClick L684–L701.
- **SF-CROPPING-025 (Stage 1 "Opening files").** Stage label = Translate(`Cropping_Open`) "Opening files" + " . . ."; for each item (row is selected/scrolled into view as it is processed): ensure the container is open; add it to a source group with (speedUpFPS, p1, p2, speed, include video = CheckBoxVideo, include audio = CheckBoxAudio). Result 1 → append "Cannot open file <file>⏎" to an error text; result 2 → append "Different format <file>⏎" (these two phrases are hard-coded English); on either error the item's p2 is set to p1 (zero length, effectively skipped). After the loop, if the error text is non-empty show it modally with caption "Error". — ButtonStartClick L704–L722.
- **SF-CROPPING-026 (Stage 2 "Generating meta info (1)").** If "Meta Info" is checked: for each item without a GPS object, auto-locate and load its GPS/G-sensor data (shared file-list cache), then count exportable points between the item's absolute start (sysTime + p1·duration) and end (sysTime + p2·duration); allocate the combined point buffer (2 × items + 2 + counted points). — ButtonStartClick L729–L748.
- **SF-CROPPING-027 (Stage 3 "Generating meta info (2)").** For each item: compute its absolute start/end, keep tMin = earliest start and tMax = latest end. If Meta or SRT is checked: export the item's points within [start, end] into a temporary track; if SRT is checked convert them to subtitle entries (offset = current length of the combined output track, scaled by the item's speed) and append SRT text to the buffer (subtitle formatting is in UnitSubtitles); if Meta is checked append the temporary track to the combined track (SF-CROPPING-033). Afterwards thin the combined track (SF-CROPPING-034) and trim it. — ButtonStartClick L755–L787.
- **SF-CROPPING-028 (Stage 4 "Generating meta info (3)").** If the combined track has points, serialise it with the "fragment" GPS saver into a binary blob to be embedded in the output container. — ButtonStartClick L794–L798.
- **SF-CROPPING-029 (Stage 5 "Copying frames").** Write the output file from the source group, embedding the GPS blob, reporting progress through DrawProgress (SF-CROPPING-031). — ButtonStartClick L805–L807.
- **SF-CROPPING-030 (finalisation, always runs).** Current row := none; DrawProgress(−1, 1); label = Translate(`Cropping_Finish`) "Writing headers and indexes" + " . . ."; repaint; **show the main window again**; release the source group. Then stage ".srt": if the subtitle buffer is non-empty write `<output path up to and including the last '.'>srt` as UTF-8 with BOM, and if tMin and tMax are > 0 set its creation time = tMin and write time = tMax. Then stage "Done": set the output's creation/write times to tMin/tMax. If the form is hidden at this point (user cancelled) delete the output file and show the form again. Finally clear the action label, restore the application if minimised, un-minimise and bring this form to front, activate it, beep, re-enable inputs, recompute totals, add the output file to the main file list (`TFormMain.AddFiles`), bring to front/activate/beep again. — ButtonStartClick L813–L821 and the uncommented tail (addresses 0x52135B–0x521566).
- **SF-CROPPING-031 (progress).** DrawProgress(timePosSec, p): if timePosSec ≥ 0, LabelTimePos = `<min(1, t/total)×100 rounded>% - <H:MM:SS of t>`; while running, map the output time to the (item index, local progress) and select that row (only when it changes: clear selection, select+focus, scroll into view; out-of-range → just scroll row 0 into view). Store progress; at most every 0.5 s (or when t < 0) refresh the current row; pump messages; return whether the form is still visible (false aborts the copy). — DrawProgress L498–L517; SelectListViewItem L475–L491.
- **SF-CROPPING-032 (stage helper).** Each stage: if the form is visible, clear row selection, focus the list, reset elapsed seconds, DrawProgress(−1, 0), pump messages, set LabelCurrentAction = Translate(stage key) + suffix + " . . .", repaint; returns visibility. Per-item helper selects the row, pumps messages and repaints the list. Exceptions inside a stage are caught and shown modally: text = `<form caption>⏎⏎<current action>[⏎⏎<current row's file>]⏎⏎<exception message>`, caption = `<form caption>. Error`; processing continues with the next stage. — NewStage L633–L647; NewItem L650–L657; ShowExept L621–L628.
- **SF-CROPPING-033 (track concatenation).** When appending an item's track to the combined track: if the combined track has no start yet (< 2.0 days), it starts at the new track's start. The time gap g = new.start − combined.finish is removed (new points shifted back by g), and point times are compressed by 1/speed relative to the combined finish. If |g| > 1 hour, the last GPS point of the existing track is flagged as a segment break. A GPS-type marker point is inserted at the old finish time before the new points and another at old finish + (new duration / speed) after them; combined finish advances accordingly. — AppendGPS L559–L602.
- **SF-CROPPING-034 (track thinning).** After concatenation keep at most one GPS point (with non-zero coordinates) per 0.21 s and one G-sensor point per 0.03 s of track time; all other point types are kept. — FilterForSpeedUp L529–L553.
- **SF-CROPPING-035 (opening from main window — selection fragment).** "Save selected fragment": remember the list size, clear list selection, un-minimise and show the form. If a file is loaded and an A–B selection exists (B > A), for each sub-file of the current group compute the local p1/p2 of A and B; if they overlap the file (p1 < 1, p2 > 0, p1 < p2) add it with p1 clamped to ≥ 0, p2 clamped to ≤ 1, its GPS object, and its start wall-clock time. If the output edit is empty, propose a name: `<copyFolder>\Fragment of <display name>` + (unless A = 0 and B = 1) ` (<HH-MM-SS> - <HH-MM-SS>)` (times of A/B in user time zone, ':' replaced by '-') + `.<extension of first sub-file>`; also set the save dialog's default extension to that extension; sanitise with filterFileName. "Fragment of " is hard-coded English. If the list was empty before, clear selection again; repaint. Without an A–B selection the form simply opens. — UnitFormMain: TFormMain.N_File_Save_VideoClick L4904–L4943.
- **SF-CROPPING-036 (opening from file list — whole group).** "Save whole group": if a file is currently loaded, temporarily set A = 0, B = 1, run SF-CROPPING-035, then reset B = 0 and re-layout. If nothing is loaded, open the form and add every sub-file of every selected list entry as a whole-file item (start time = the sub-file's recorded start); clear selection if the list was empty before; repaint. — UnitFormMain: TFormMain.N_FileList_SaveClick L4949–L4972.
- **SF-CROPPING-037 (unit init/final).** Initialization/Finalization only maintain a unit reference counter — no behaviour. — UnitFormCropping: initialization L938.

---

### 6.6 TFormCroppingItem (UnitFormCroppingItem) — edit one crop item

**Purpose.** Modal editor for one TCropItem: start/end as fraction or time, playback speed and speed-up method.
**Opened from.** TFormCropping Edit (SF-CROPPING-017). Singleton created via `FormCroppingItem()` with TFormCropping as parent (owned, centred over it) — UnitFormCroppingItem: FormCroppingItem L62–L64. Re-translated by main DoTranslate (UnitFormMain L4798–L4806, L4618–L4619).
**Inputs/outputs.** Reads and writes the passed TCropItem in place on OK (p1, p2, speed, speedUpFPS); Cancel leaves it unchanged.

**Layout** (DFM: bsDialog, BorderWidth 10, ClientWidth 312, ClientHeight 364; caption = Translate(`Cropping_Title`) + ". " + Translate(`N_Edit`) → "Cutting. Edit"; form auto-sizes after runtime re-layout).

| Control | Class | L/T/W/H | Caption/text (key → English) | Notes |
|---|---|---|---|---|
| LabelFileName | TLabel | 24/0/70/13 → Left 0 at runtime | source file path | |
| LabelDuration | TLabel | 24/16/40/13 → Left 0 | `Duration` + ":" "Duration:" | |
| LabelDurationValue | TLabel | 72/16/93/13 | full duration `H:MM:SS` | placed 4 px right of LabelDuration |
| LabelStart | TLabel | 24/44/22/13 | `Started` + ":" "Started:" | right-aligned 4 px left of EditTime1 |
| EditTime1 | TEdit | 56/40/57/21 | start as time offset `H:MM:SS` | |
| LabelDateTime1 | TLabel | 120/44/102/13 | absolute start "date  time" | |
| EditP1 | TEdit | 248/40/65/21 | start fraction, 6 decimals (DFM "0.000000") | |
| LabelFinish | TLabel | 24/76/27/13 | `Finished` + ":" "Finished:" | |
| EditTime2 | TEdit | 56/72/57/21 | end as time offset | |
| LabelDateTime2 | TLabel | 120/76/102/13 | absolute end | |
| EditP2 | TEdit | 248/72/65/21 | end fraction (DFM "1.000000") | |
| LabelSpeed | TLabel | 0/118/31/13 | `Speed` + ":" "Speed:" | right-aligned 4 px left of track bar |
| TrackBarSpeed | TTrackBar | 40/104/217/33 | preset index | ticks top/left, no tab stop |
| EditSpeed | TEdit | 264/112/49/21 | "N" or "1/N" (DFM "1/16") | |
| ButtonOk | TButton | 32/160/75/25 | "Ok" (not translated) | Default; centred pair with Cancel |
| ButtonCancel | TButton | 128/160/75/25 | `Cancel` "Cancel" | Cancel=true (Esc) |
| SpeedUpMethod | TGroupBox | 16/200/241/153 → full client width | `Cropping_SpeedUpMethod` "Speed up method (no recompression are made)" | visible only when speed > 1 |
| SUM_FPS | TRadioButton | 5/16/18/18 | — | "keep all frames" |
| SUM_FPS_Caption | TLabel | 28/16/153/65 (no autosize, word-wrapped) | `Cropping_BigFPS` with `%fps%` replaced | click selects SUM_FPS |
| SUM_KeyFrames | TRadioButton | 5/81/18/18 | — | "key frames only" |
| SUM_KeyFrames_Caption | TLabel | 28/81/174/65 | `Cropping_KeyFrameOnly` with `%key%` replaced | click selects SUM_KeyFrames |

English texts: Cropping_BigFPS = "Keep all frames and reduce time interval between them. Computer must have enough power to play this video. \nResult: %fps% fps."; Cropping_KeyFrameOnly = "Keyframes only. Video will be light and easy to play in any player. \nDetected keyframe every %key% frames." (`\n` = line break).

**Requirements**

- **SF-CROPPINGITEM-001 (create).** Clears the "change in progress" re-entrancy flag. — FormCreate L71–L73.
- **SF-CROPPINGITEM-002 (speed presets).** On show, build the speed preset list (track-bar positions 0..n−1): always 1/8, 1/4, 1/2, 1; then depending on the GOP pattern string of the video:
  - "PPPPPPPIPPPPPPPIPPPPPPPIPPPPPPP" (I every 8): 4, 8, 16, 32, 64, 128, 512, 1024; key interval 8.
  - "BBPBBPBBPBBPBBI…" (the 59-char pattern, I every 15): 2, 3, 8, 15, 30, 60, 120, 480, 1020; key interval 15.
  - "BBBPBBBP…BBBI…" (the 127-char pattern, I every 32): 2, 3, 4, 16, 32, 64, 128, 256, 1024; key interval 32.
  - empty GOP string: 2, 3, 4, 8, 15, 30, 60, 120; key interval 0.
  - any other non-empty pattern of length L: n = L+1; presets n div 2, n, 2n, 4n, 8n, 16n; key interval n.
  Set SUM_KeyFrames_Caption from `Cropping_KeyFrameOnly` with `%key%` = key interval; track-bar max = count−1. — AddS L79–L82; AddA L86–L91; Show L95–L123.
- **SF-CROPPINGITEM-003 (show — fill and layout).** Show the file path and full duration; temporarily set P1 "0", P2 "1", speed "1"; lay out labels (captions right-aligned 4 px left of their edits, file/duration labels at left 0, duration value 4 px right of its caption, OK/Cancel centred with BorderWidth gap, method group full width, radio captions fill to the right edge); then set P1/P2 to the item's values with 6 decimals (change events update the time edits and date labels), render the item's speed (SF-CROPPINGITEM-005), check SUM_FPS if speedUpFPS else SUM_KeyFrames, auto-size, and show **modally**. — Show L125–L167.
- **SF-CROPPINGITEM-004 (OK).** p1 := clamp(float(EditP1), 0, 1); p2 := clamp(float(EditP2), 0, 1); speed := parsed EditSpeed (SF-CROPPINGITEM-006), and if ≤ 0 then 1; speedUpFPS := SUM_FPS checked; close. No check that p1 < p2 (an item with p2 ≤ p1 produces zero length). A non-numeric P1/P2 raises a conversion error (application-level exception message). — ButtonOkClick L176–L183.
- **SF-CROPPINGITEM-005 (render speed).** Speed < 1 → "1/" + round(1/speed); otherwise round(speed) as integer text, written into EditSpeed. — DrawSpeed L187–L191.
- **SF-CROPPINGITEM-006 (parse speed).** EditSpeed text starting with "1/" → 1/N where N = int(rest, default −1) only if N > 0 (else −1); otherwise int(text, default −1). — ReadSpeed L202–L208.
- **SF-CROPPINGITEM-007 (track bar → edit).** Moving the track bar writes the preset value at that position into EditSpeed. — TrackBarSpeedChange L195–L199.
- **SF-CROPPINGITEM-008 (edit → track bar, method box).** Any change of EditSpeed: if the parsed value equals a preset, move the track bar there; the speed-up-method group is visible only when speed > 1; when visible, SUM_FPS_Caption = `Cropping_BigFPS` with `%fps%` = round(speed × item fps), and SUM_KeyFrames is (re)checked. — EditSpeedChange L216–L226.
- **SF-CROPPINGITEM-009 (P1/P2 → time).** Changing EditP1 (resp. EditP2): t = fraction × duration; LabelDateTime1 (2) = date + two spaces + time of (item start wall-clock + t + user time-zone shift); unless the change came from the time edit, EditTime1 (2) = t as H:MM:SS. — EditP1Change L233–L240; EditP2Change L246–L253.
- **SF-CROPPINGITEM-010 (time → P1/P2).** Changing EditTime1 (resp. EditTime2), unless the change came from the P edit: parse as time (default 0 for start, full duration for end) and write fraction = time / duration with 6 decimals into EditP1 (2). The shared flag prevents ping-pong between the two edits. — EditTime1Change L257–L261; EditTime2Change L265–L269.
- **SF-CROPPINGITEM-011 (caption clicks).** Clicking the descriptive label of a method checks its radio button. — SUM_FPS_CaptionClick L273–L275; SUM_KeyFrames_CaptionClick L278–L280.
- **SF-CROPPINGITEM-012 (Cancel).** Closes without changing the item. — ButtonCancelClick L171–L173.

---

### 6.7 TFormFileProperty (UnitFormFileProperty) — "Property"

**Purpose.** Read-only (copyable) two-column property sheet of one file-list entry and, for grouped entries, of each of its sub-files.
**Opened from.** File-list popup `N_FileList_Property` "Property" (no shortcut) → `TFormMain.N_FileList_PropertyClick` (UnitFormMain L3962–L3965): opens **one new window per selected entry** (non-singleton).
**Inputs.** TFileItem (display name, duration text, group record, sub-items). **Outputs.** Clipboard text on demand.

**Layout** (DFM: Width 441, Height 494, sizeable, Position poDefaultPosOnly; caption = Translate(`Property_Title`) "Property" + " " + display name; icon `icon_property.ico`; own taskbar button; UI font applied).

| Control | Class | L/T/W/H | Align | Notes |
|---|---|---|---|---|
| StringGrid | TStringGrid | 0/0/425/415 | alClient | 2 columns, first column fixed (grey), no fixed row, default col width 100, row height 16, editable cells (so text can be selected), vertical scrollbar, no border; starts with 0 rows |
| Panel1 | TPanel | 0/415/425/41 | alBottom | bevel none |
| ButtonCopy | TButton | 8/8/217/25 | in Panel1 | Translate(`Property_CopyAll`) "Copy All to Clipboard" |

**Requirements**

- **SF-FILEPROPERTY-001 (create).** Load icon, own taskbar button, apply UI font, empty the grid. — FormCreate L32–L37.
- **SF-FILEPROPERTY-002 (resize).** Second column width = grid client width − 1 − first column width. — FormResize L40–L44.
- **SF-FILEPROPERTY-003 (add row).** Each row: column 0 = current indent prefix + Translate(name) (empty name allowed), column 1 = value; the first row reuses the initial empty row; column 0 widens to fit the longest label (+4 px). — Add L54–L62.
- **SF-FILEPROPERTY-004 (header rows).** Rows: `Name` "Name" = display name; `Duration` "Duration" = list duration text; `DateTime` "Date and Time" = start (finish − duration of the group video, plus user time-zone shift) as date-time. — Popup L144–L149.
- **SF-FILEPROPERTY-005 (single vs grouped).** One sub-file → its block (SF-FILEPROPERTY-006) follows directly. Several → row `Grouped` "Grouped" = count, then the group's combined block, then for each sub-file: a blank row and its block with prefix "[k] " (k from 1). The window is shown modeless. — Popup L151–L163.
- **SF-FILEPROPERTY-006 (sub-file block).** Rows (indented 4 spaces under the block): `File` = full video path; one unnamed row per media-info line; `Size` = "<bytes> byte   (<MiB, 1 decimal> Mb)" (keys `byte`, `Mb`); `Locked` = Translate(`Locked_true`) "true" when fully locked, Translate(`Locked_partial`) "partial" when partially, row omitted otherwise; then start/finish/duration rows (SF-FILEPROPERTY-008) for the video, where Started/Finished values are prefixed "(<position, 6 decimals>) " when the time block has a non-trivial in-file position (start position ≠ 0 or end position not 0/1), and the Duration value is prefixed "~" when the duration is not exactly known (duration state ≠ 3). Hidden debug row: if the mouse pointer is at screen X = 0 when the window opens, an extra unnamed row shows the GOP pattern string and its length+1. — AddSubitem L85–L102.
- **SF-FILEPROPERTY-007 (meta info / GPS log sub-blocks).** If the sub-file has a meta/info file: row `MetaInfo` "Meta Info" (no value), then indented: `File` = path; `DataFormat` "Format" = GPS format id (if any); "GPS " + Translate(`points`) = GPS point count (if > 0); "G-Sensor " + Translate(`points`) = G-sensor count (if > 0); if GPS status = 2 (valid track): `Property_Track_Length` "Track Length" and `AvgSpeed` "Speed (avg)" — metric: "<round(m)> m" and "<x.x> km/h"; British units: "<km/1.609344, 3 decimals> miles" and "<x.x> mph" (keys `Meters`, `KmetersPerH`, `Miles`, `MilesPerH`); then Started/Finished/Duration of the meta time block. If the sub-file has a separate GPS log: row "GPS Log" (key `GPS Log`), then indented `File` and its Started/Finished/Duration. — AddSubitem L104–L136.
- **SF-FILEPROPERTY-008 (start/finish rows).** Started (`Started`) only if start > 0: date-time of start + time-zone shift; Finished (`Finished`) only if finish > 0; Duration (`Duration`) only if start > 0: finish − start as duration text (with "." millisecond separator option per caller). — AddStartFinish L73–L82.
- **SF-FILEPROPERTY-009 (copy all).** Build text: for each row, cells joined by TAB, rows terminated by CRLF, and put it on the clipboard using the current UI language's code page. — ButtonCopyClick L171–L180.
- **SF-FILEPROPERTY-010 (close).** Closing frees the window. — FormClose L47–L49.
- **SF-FILEPROPERTY-011 (unit init/final).** Reference counter only. — UnitFormFileProperty: initialization L183.

---

### 6.8 TFormRepair (UnitFormRepair) — "Repair broken file"

- **Purpose:** Recover a dashcam MOV/MP4 file that was not finalised, e.g. after power loss. It offers two methods:
  - **Fast** — in-process MOV/MP4 header rebuild (TFileRecoverHeader). It writes **into the broken file itself**.
  - **Full** — the embedded external tool `Repairer.exe` (TFileRecoverMD). It writes to a separate output file.

  Both methods need a reference file: a good recording made with the same camera settings.
- **Opened from:**
  - Main menu `N_File_Repair_Video` → `Open(nil, nil)`, which opens with empty fields (UnitFormMain: TFormMain.N_File_Repair_VideoClick L4976–4978).
  - File-list popup `N_FileList_Repair` (caption `%N_File_Repair_Video%`). If a list row is current, it picks the row's first sub-file whose duration state is 2, falling back to sub-file 0, and calls `Open(thatFile, fileManager)` (UnitFormMain: TFormMain.N_FileList_RepairClick L4983–4992). Duration state 2 is presumed to mean "duration could not be read"; 3 means exact and 1 means approximate.
- **Outputs:**
  - the repaired file, with the broken file's original creation and modification times restored;
  - the repaired file is added to the main file list (`TFormMain.AddFiles`);
  - the registry value `copyFolder` is updated (shared with the copy/cropping features).

**Layout (DFM):** sizeable, `BorderWidth=16`, design 435×439. Caption = `Repair_Title` ("Repairing"). Minimum height is locked to the design height (UnitFormRepair: TFormRepair.FormCreate L68).

| Control | Class | Parent | L,T,W,H | Caption key → English / notes |
|---|---|---|---|---|
| LabelBroken | TLabel | form | 0,0 | `Repair_BrokenFile:` "Broken file:" |
| EditBroken | TEdit | form | 0,16,353,21 | broken file path; double-click = browse |
| ButtonFileBorken | TButton | form | 360,16,21,21 | `...` |
| LabelRef | TLabel | form | 0,56 | `Repair_RefFile:` "Good file, recorded with same parameters:" |
| EditRef | TEdit | form | 0,72,353,21 | reference file path; double-click = browse |
| ButtonFileRef | TButton | form | 360,72,21,21 | `...` |
| GroupBoxMethod | TGroupBox | form | 0,120,377,161 | `Repair_Method` "Method" |
| RadioButtonFast | TRadioButton | group | 10,24,18,18 | **Checked by default** |
| LabelCaptionFast | TLabel | group | 32,24,185,53 | word-wrap; `Repair_MethodFast` ("Find and recover file's header inside the file.\nWorks for MOV and MP4 files, recorded by most common dashcams on Ambarella chip.") |
| RadioButtonFull | TRadioButton | group | 10,76,18,18 | |
| LabelCaptionFull | TLabel | group | 32,76,201,53 | word-wrap; `Repair_MethodFull` ("Full file scan and trying to decode all bytes.\nThis generates new file, that contains all valid frames.") |
| LabelResult | TLabel | group | 24,132 | `Repair_FullOutputFile:` "Result:" |
| EditResult | TEdit | group | 64,128,265,21 | output path (Full method only); double-click = browse |
| ButtonFileResult | TButton | group | 336,128,21,21 | `...` |
| ButtonRepair | TButton | form | 96,296,201,33 | `Repair_BtnStart` "Do Recover" |
| LabelStatus | TLabel | form | 0,336 | progress text; hidden when idle |
| ProgressBar | TProgressBar | form | Align=alBottom, H=17 | hidden when idle |
| TimerConsole | TTimer | — | Interval 50 ms | progress polling |

(Captions: UnitFormMain: TFormMain.DoTranslate L4814–4822.)

**Requirements**

- **SF-REPAIR-001** Creation sets the icon `icon_repair.ico`, locks the minimum height and clears the recoverer reference. (UnitFormRepair: TFormRepair.FormCreate L66–70)
- **SF-REPAIR-002** Resize behaviour (responsive layout):
  - the `...` buttons are right-aligned and each edit stretches up to its button;
  - the group box takes the full width;
  - both method descriptions take the group's width minus 8;
  - the Result `...` button is right-aligned to the description's right edge;
  - EditResult spans from LabelResult's right edge + 4 to that button;
  - LabelStatus sits 4 px above the progress bar;
  - ButtonRepair is centred horizontally.

  (UnitFormRepair: TFormRepair.FormResize L73–86)
- **SF-REPAIR-003** `Open(file, list)` clears all three edits, then:
  - If a file is given, its path goes to EditBroken. The app then **auto-picks a reference file** from the current file list:
    - Candidates are list sub-files other than the broken file that have an exact duration (state 3) and do not contain `_R.` in the file name (`_R.` = rear-camera naming).
    - The candidate with the smallest time distance between its start/end times and the broken file's start/end times wins. The initial threshold is 999 999.
    - The winner's path goes to EditRef.
  - The UI is then enabled, the Fast method is selected and the form is shown non-modally.

  (UnitFormRepair: TFormRepair.Open L93–122)
- **SF-REPAIR-004** `SetEnable(e)` toggles between the idle and running states:
  - Idle (e = true): repair button, the three edits and the method group are enabled; the progress bar and status label are hidden; the polling timer is stopped.
  - Running (e = false): the reverse.

  It re-runs the layout afterwards. (UnitFormRepair: TFormRepair.SetEnable L126–136)
- **SF-REPAIR-005** Selecting Fast (radio or its label) checks the radio and **disables** LabelResult, EditResult and the Result `...` button. Selecting Full enables them. Clicking LabelResult also selects Full. (UnitFormRepair: LabelCaptionFastClick L272–277, LabelCaptionFullClick L280–285)
- **SF-REPAIR-006** Browse for broken/reference file. The `...` buttons and double-clicks on the broken or reference edit open the main form's shared open dialog `OpenDialogFileAdder` (filter "DashCam Files (*.mov, *.mp4)|*.mov;*.mp4;*.mp4v|Video Files|…|Any Files|*.*"), pre-filled with the current text. On OK, if the target is the broken-file edit, EditResult is cleared first; then the chosen path is written. (UnitFormRepair: ButtonFileBorkenClick L290–304)
- **SF-REPAIR-007** Browse for result. The `...` button or a double-click opens the Save dialog borrowed from FormCropping (DefaultExt `mp4`; filter "MP4 and MOV files|*.mp4;*.mov|Any Files|*.*"; overwrite prompt), pre-filled with EditResult. On OK the path is written and its folder is saved to the registry value **`copyFolder`**. (UnitFormRepair: ButtonFileResultClick L307–315)
- **SF-REPAIR-008** Default output name. When EditBroken changes and EditResult is empty, the app proposes `<copyFolder>\<name>`:
  - `<name>` is the broken file name; if that name is 12+ characters long, its first character is replaced by `_`.
  - `copyFolder` comes from the registry (default `C:\`) and gets a trailing backslash if missing.

  (UnitFormRepair: EditBrokenChange L320–330)
- **SF-REPAIR-009** Validation on "Do Recover", in order. Every failure shows a modal error (caption `Error`) and focuses the offending edit.
  1. The broken file is empty or missing → `Mes_FileNotExists` + `:\r\n` + `Repair_BrokenFile`. (L154–157)
  2. The reference file is empty, missing, or cannot be opened by the app's own container parser (`CheckFile` → `LoadSourceContainerFile`) → `Mes_FileNotExists:\r\n Repair_RefFile`. (L160–163; CheckFile L145–148)
  3. Broken path equals reference path (case-insensitive) → `Repair_BrokenFile   =   Repair_RefFile`. (L166–169)
  4. Broken file size is 0 → text `File size: 0`. (L172–174)

  (UnitFormRepair: TFormRepair.ButtonRepairClick L150–174)
- **SF-REPAIR-010** **32 MiB crash heuristic.** If the broken file size is an exact multiple of 32 MiB (33 554 432), a warning `MessageDlg` (Yes/No) is shown.
  - Text: the file path, a blank line, then `Repair_PrepareWarning` with `%cmd%` replaced by `chkdsk <drive>: /f` (drive = first letter of the path).
  - **No** → `chkdsk <drive>: /f` is launched elevated ("run as admin") and the repair is aborted.
  - **Yes** → the repair continues.
  - Any other result → abort.

  (UnitFormRepair: ButtonRepairClick L177–182)
- **SF-REPAIR-011** If the broken file is detected as encrypted (`MOVCheckEncrypted`), show `Error` / `Mes_FileEncrypted` ("File is encrypted") and abort. (UnitFormRepair: ButtonRepairClick L186–188)
- **SF-REPAIR-012** Full method only, extra checks:
  - Result path empty → `Repair_FullOutputFile` error, focus EditResult.
  - Result equals the broken path → `Repair_FullOutputFile   =   Repair_BrokenFile`.
  - Result equals the reference path → `Repair_FullOutputFile   =   Repair_RefFile`.
  - Result file exists → Yes/No confirmation `Mes_FileExists` / path / `Mes_SureToReplace ?`; anything but Yes aborts.

  (UnitFormRepair: ButtonRepairClick L191–216)
- **SF-REPAIR-013** Start:
  1. Create the recoverer (Full → external-tool wrapper; Fast → in-process header rebuild).
  2. Remember the broken file's creation and last-write times.
  3. Start the recoverer with (broken, reference, result).
  4. Enable the 50 ms poll timer and switch the UI to the running state.

  (UnitFormRepair: ButtonRepairClick L216–225)
- **SF-REPAIR-014** Progress polling, every 50 ms while the form is visible:
  - Ask the recoverer for (percent, framesFound). While it reports "running", set the progress bar position (0–100) and LabelStatus to `<percent>%    <Repair_FramesFound>: <frames>` (EN "Frames found").
  - When it reports finished, or the form was closed: stop the timer and free the recoverer. Freeing kills a still-running external process and deletes its temporary EXE.

  (UnitFormRepair: TimerConsoleTimer L232–242)
- **SF-REPAIR-015** Completion, only if the form is still visible:
  1. Restore the application if minimised, un-minimise the form, bring it to front, activate it and beep.
  2. Pick the output: the **broken file itself** for Fast, EditResult for Full.
  3. Write back the saved creation/write times to the output and add the output to the main file list.
  4. If frames < 1, show `Error` / `Repair_Fail` ("Cannot recover this file"). Otherwise show caption = form caption, text = `Repair_Ok` + blank line + `Repair_FramesFound: <n>`.
  5. Return the UI to the idle state.

  (UnitFormRepair: TimerConsoleTimer L244–269)
- **SF-REPAIR-016** **External tool (Full method), user-visible behaviour only:**
  1. Clear the read-only flag of the output and create the output folder.
  2. **Free-space check:** the output volume needs more than 1.1 × broken size + 16 MiB free. Otherwise show `Error` / `Mes_NoFreeSpace` with the required and available sizes in `Mb`, and do not start.
  3. Decode the RCDATA resource `REPAIRER` (byte *i*, 1-based, is XORed with *i* mod 256).
  4. Write it to `%TEMP%\<exe base name> Repairer.exe`.
  5. Run it hidden with stdout/stderr piped: `"<tmp exe>" -i "<broken>" -o "<output>" -nfd -ref "<reference>" -v 10`. If the launch fails, show `Error` / `Mes_CannotRun` + blank line + exe path.
  6. Read progress from the tool's console output:
     - lines starting with `ofs=<n>,` give the byte offset (percent = offset / broken size);
     - every line containing `chunk at` or `sample at` counts as a found frame.
  7. Finished = process exited. On teardown the process is terminated if still alive and the temp EXE is deleted.

  (UnitFileRecoverMD: TFileRecoverMD.Init L41–96, Destroy L100–113, GetRepairerModule L118–122, GetStatus L128–156, OnConsoleRead L161–172)
- **SF-REPAIR-017** Fast method: a background thread reads the reference file's header and rebuilds the broken file's `moov` in place. Progress = thread percent; frames = video + audio frames found. Internals are out of scope here. (UnitFileRecoverHeader: TFileRecoverHeader.Init L65–66, GetStatus L75–82)

---

### 6.9 TFormSetupDevice (UnitFormSetupDevice) — "Setup Device"

- **Purpose:** Edit the binary `setup.cfg` on the memory card of a DOD GS600 / Carcam Q3-class camera. Fields: video frame size, time zone, and G-sensor sensitivity on X/Y/Z.
- **Opened from:** main menu `N_SetupDevice` → non-modal `Show` (UnitFormMain: TFormMain.N_SetupDeviceClick L5211–5213).
- **I/O:** reads and writes `<drive>:\setup.cfg` (raw binary, no text encoding).

**Layout (DFM):** `BorderStyle=bsSingle`, `BorderIcons=[biSystemMenu]`, `BorderWidth=10`, `AutoSize`, design client 325×219. Caption `SetupDevice_Title` ("Setup Device"). (UnitFormMain: DoTranslate L4769–4777)

| Control | Class | L,T,W,H | Caption key / items / defaults |
|---|---|---|---|
| ImageDod | TImage | 0,0,50,23 | bitmap `Form_SetupDeivce_dod.bmp` (sic) |
| LabelDevice | TLabel | 69,6 | `SetupDevice_Device:` "Disk:" |
| ComboBoxDevice | TComboBox (drop-down list) | 96,2,229,21 | drive list (design item `T:\`) |
| Bevel1 | TBevel | 36,34,265,9 | separator line, stretched to full width at runtime |
| LabelFrameSize | TLabel | 31,54 | `PictureSize:` "Picture Size:" |
| ComboBoxFrameSize | TComboBox (list), Courier New | 96,50,229,22 | disabled; items below |
| LabelTimeZone | TLabel | 28,86 | `SetupDevice_TimeZone:` "Time Zone:" |
| ComboBoxTimeZone | TComboBox (list), Courier New | 96,82,81,22 | `GMT-12` … `GMT+0` … `GMT+12` (25 items) |
| LabelGSensor | TLabel | 20,130 | `SetupDevice_GSensor` "G-Sensor\nsensitivity" |
| LabelXCaption / LabelYCaption / LabelZCaption | TLabel | 80,114 / 80,139 / 80,164 | `X`, `Y`, `Z` |
| GSensorX / GSensorY / GSensorZ | TTrackBar | 88,112 / 88,137 / 88,162; 217×25 | range 0–10, PageSize 1, disabled |
| LabelX / LabelY / LabelZ | TLabel | 307,114 / 307,139 / 307,164 | current value |
| ButtonWrite | TButton | 68,194,75,25 | `SetupDevice_BtnWrite` "Write"; Default; disabled |
| ButtonCancel | TButton | 156,194,75,25 | `Cancel`; Cancel (Esc) |

Frame-size items (fixed English text in the DFM) and their device codes (`frameSizeIndexes`):

| # | Item text | code written |
|---|---|---|
| 0 | `1080P   1920x1080   30fps` | 5 |
| 1 | `1080P   1440x1080   30fps` | 11 |
| 2 | ` 720P   1280x720    30fps` | 17 |
| 3 | ` WVGA    848x480    60fps` | 25 |
| 4 | ` WVGA    848x480    30fps` | 27 |
| 5 | `  VGA    640x480    30fps` | 29 |
| 6 | ` QVGA    320x240    30fps` | 31 |
| 7 | `< Custom >` → shown as `SetupDevice_OtherSize (<code>)` ("Special (n)") | any byte |

**setup.cfg format** (as far as this dialog touches it): a binary blob read and written whole. The default is 128 bytes.

| Offset | Meaning | Encoding |
|---|---|---|
| 6 | frame size code | byte (see table). On read, code *c* also matches table entry with value *c*+1. |
| 7 | time zone | signed byte, hours −12…+12 (combo index − 12) |
| 8 / 9 / 10 | G-sensor X / Y / Z | byte 0–10 (clamped on read) |
| others | preserved unchanged | — |

Default blob used when no file exists (hex, 128 bytes): `0E 01 52 50 F1 01 05 04`, then zeros up to byte 122, then `00 50 0E 05 53` at offsets 123–127. Defaults: 1080P/30, GMT+4, G-sensor 0/0/0.

**Requirements**

- **SF-SETUPDEVICE-001** On creation: icon `icon_setupDevice.ico`, the DOD bitmap is loaded, and the time-zone combo is filled with `GMT-12` … `GMT-1`, `GMT+0` … `GMT+12`. (UnitFormSetupDevice: TFormSetupDevice.FormCreate L66–76)
- **SF-SETUPDEVICE-002** Every show:
  1. Disable all editors.
  2. Right-align each caption label 5 px left of its control (G-sensor label 10 px), and vertically centre it on the Y row.
  3. Make the bevel full width and place the DOD image 10 px left of the Disk label.

  (UnitFormSetupDevice: FormShow L84–93)
- **SF-SETUPDEVICE-003** **Drive detection.** On every show the device list is rebuilt from drive letters A…Z that are valid **USB flash drives** (`IsValidUSBFlashDrive`) and whose disk info is readable. Each item reads `X:   <total GB, rounded up> Gb  (<free GB, 1 decimal> Gb free)   <volume label>`. If exactly one drive is found it is auto-selected and loaded; otherwise nothing is selected. The custom frame-size item is relabelled `SetupDevice_OtherSize`. (UnitFormSetupDevice: FormShow L94–110)
- **SF-SETUPDEVICE-004** Selecting a drive reads `<letter>:\setup.cfg` completely. If it is missing or empty, a Yes/No confirmation is shown: drive text + blank line + `SetupDevice_NoConfig`. **Yes** loads the 128-byte default blob. If there is no data, editors stay disabled. (UnitFormSetupDevice: ComboBoxDeviceChange L139–158, L178)
- **SF-SETUPDEVICE-005** With data loaded:
  - Enable the editors.
  - Select the frame-size row whose code equals byte 6 or byte 6 + 1; if none matches, select the Custom row.
  - Set the time zone combo to byte 7 + 12 and the sliders to bytes 8/9/10 (clamped 0–10).
  - Refresh the value labels.

  (UnitFormSetupDevice: ComboBoxDeviceChange L159–176)
- **SF-SETUPDEVICE-006** Frame-size change, with data loaded (≥ 10 bytes):
  - A table row writes its code into byte 6.
  - Choosing Custom *by the user* opens an input box. Caption: `<Picture Size label> <item text>`. Prompt: `SetupDevice_EnterOther:` ("Enter code to set frame resolution"). Default: the current byte 6. The entered integer (5 if not a number) goes into byte 6.
  - The Custom item text is always refreshed to `SetupDevice_OtherSize (<byte 6>)`. If nothing is selected, Custom is selected.

  (UnitFormSetupDevice: ComboBoxFrameSizeChange L236–251)
- **SF-SETUPDEVICE-007** Moving any slider updates LabelX/Y/Z with the slider positions. (UnitFormSetupDevice: GSensorXChange L198–202)
- **SF-SETUPDEVICE-008** Write:
  1. If no frame size is selected, show a message whose text is the Picture Size label text and whose caption is the form caption, then stop.
  2. Store time zone (index − 12) and X/Y/Z into bytes 7–10.
  3. Create `setup.cfg` if missing, open it for writing, truncate it to the blob length and write the blob.
  4. Show `Ok` (caption = form caption) and close the form.

  (UnitFormSetupDevice: ButtonWriteClick L207–231)
- **SF-SETUPDEVICE-009** (trivial) Cancel closes the form. `SetEnab(v)` enables/disables frame size, time zone, the 3 sliders and Write. (UnitFormSetupDevice: ButtonCancelClick L193–195; SetEnab L183–190)

---

### 6.10 TFormUpdateExe (UnitFormUpdateExe) — application self-update

- **Purpose:** Offer the new program version found by the background update check (6.12). It can download and replace the running EXE ("automatically") or open the download page in a browser.
- **Opened from:** only from the background checker, via `TAutoUpdateChecker.ShowFormUpdateExe` (UnitServerInfo: ShowFormUpdateExe L123–125), as a non-modal `Show`.
- **Inputs:** the globals `serverVersionText`, `serverUrl` and `serverUrlWeb` (parsed from the server INI) and `Self_Version`.
- **Output:**
  - the replaced EXE plus a restart;
  - the session flag `descardUpdate := true`, which suppresses re-offering in this session. This flag is **not persisted**.

**Layout (DFM):** `BorderStyle=bsSingle`, `BorderIcons=[biSystemMenu]`, `BorderWidth=20`, `AutoSize`, design client 443×161. Caption `Update_Title` ("Update"). (UnitFormMain: DoTranslate L4717–4724)

| Control | Class | L,T,W,H | Caption key / notes |
|---|---|---|---|
| ImageLeft | TImage | 0,0,124,124 | `Form_Update_left.bmp` |
| ImageRight | TImage | 319,0,124,124 | `Form_Update_right.bmp`; right-aligned at runtime |
| Label1 | TLabel | 128,24,185,16 | taCenter, 10pt; `Update_NewVersion` "The new version is ready" |
| LabelVersion | TLabel | 128,42,185,16 | taCenter, 10pt; = `serverVersionText` |
| Label2 | TLabel | 128,64,185,41 | taCenter, wrap; `Update_WantDownload ?` "Do you want to update ?" |
| ProgressBar | TProgressBar | 128,105,121,17 | hidden; stretched between the images at runtime |
| LabelProgress | TLabel | 172,124 | hidden; `0 Kb (0%)` |
| ButtonAuto | TButton | 0,136,169,25 | Default; `Update_Btn_Auto` "Yes, do it automatically" |
| ButtonBrowser | TButton | 184,136,169,25 | `Update_Btn_Browser` "Download in browser" |
| ButtonCancel | TButton | 368,136,75,25 | Cancel (Esc); `Cancel` |

**Requirements**

- **SF-UPDATEEXE-001** On show:
  - On first show, load the icon `icon_update.ico` and the left/right bitmaps.
  - Right-align the right image.
  - Set the form colour to the top-left pixel of the left bitmap.
  - Stretch the progress bar between the images and reset it to 0.
  - Set LabelVersion to `serverVersionText`.
  - Place LabelProgress just right of ButtonAuto, vertically centred on it.

  (UnitFormUpdateExe: TFormUpdateExe.FormShow L48–65)
- **SF-UPDATEEXE-002** Cancel sets `descardUpdate`, closes the form and re-shows the main form. (UnitFormUpdateExe: ButtonCancelClick L68–72)
- **SF-UPDATEEXE-003** Browser sets `descardUpdate`, opens `serverUrlWeb` (the INI `downloadweb`, or `downloadurl` if absent) with the shell "open" verb, and closes the form. (UnitFormUpdateExe: ButtonBrowserClick L75–79)
- **SF-UPDATEEXE-004** **EXE naming helper `fn(ver)`:**
  1. Take the running EXE's full path.
  2. If it already carries a `_v<x.y>` suffix (the two characters `_v` at position len−8), remove those 5 characters.
  3. If `ver` is not empty, insert `_v<ver>` before `.exe`.

  Example: `C:\RV\RegistratorViewer.exe` + ver `6.1` → `C:\RV\RegistratorViewer_v6.1.exe`. (UnitFormUpdateExe: fn L88–93)
- **SF-UPDATEEXE-005** "Automatically", preparation:
  1. Delete any stale `fn(serverVersionText)`, then create it fresh. On failure show `Cannot create file\r\n<path>` with caption `<form caption>. Error` and stop.
  2. Set `descardUpdate`.
  3. Disable Auto, hide Browser, show the progress bar and progress label.
  4. Stop playback in the main window (`ButtonStopClick`) and **hide the main window**.

  (UnitFormUpdateExe: ButtonAutoClick L95–111)
- **SF-UPDATEEXE-006** Download:
  - HTTP GET of `serverUrl` (the INI `downloadurl`), pumping UI messages, until the response is complete or the user closes the form.
  - Progress callback text: `<Kb> Kb   ( <percent>% )`. When the total is known, the progress bar shows bytes received / total. Updates arrive at most every 0.1 s (SF-NET-004).

  (UnitFormUpdateExe: ButtonAutoClick L114–119; onStatus L160–167)
- **SF-UPDATEEXE-007** Download failure. In each case the main window is re-shown:
  - Empty response, or the form was closed → delete the partial file. If the form is still visible, show text `Error` + `\r\n\r\nСannot download` (sic: the first letter is Cyrillic) with the form caption as caption.
  - Response shorter than 102 400 bytes → show `Error` + `\r\n\r\nMissing data (<len>)`.

  (UnitFormUpdateExe: ButtonAutoClick L122–126, L145–149)
- **SF-UPDATEEXE-008** **Install (success path).** The downloaded payload is used as-is: it is a raw EXE, not an archive. `Update_Err_Decompress` is not used here.
  1. Write the payload to `fn(serverVersionText)`.
  2. `canonical = fn('')` (EXE name without version suffix). `backup = fn(Self_Version)`; append `_` while `backup` exists.
  3. Rename the running EXE `canonical` → `backup`, then rename the downloaded file → `canonical`.
  4. Launch `canonical` with arguments `/whatsnew "/del<backup>"`.
  5. Mark `backup` hidden and try to delete it (this normally fails while it is still running).
  6. Terminate the application.

  The new instance opens What's New (arg 1 = `/whatsnew`) and deletes the file named after `/del` in arg 2 (UnitFormMain: TFormMain.TimerOnStartTimer L5188–5191). (UnitFormUpdateExe: ButtonAutoClick L129–143)
- **SF-UPDATEEXE-009** In all non-terminating paths the form finally hides itself and releases the file stream. (UnitFormUpdateExe: ButtonAutoClick L152–153)

---

### 6.11 TFormUpdateFirmware (UnitFormUpdateFirmware) — dash-cam firmware update

- **Purpose:** When a supported dash-cam memory card is found during a file search, and the update server says newer firmware exists for that model, offer to download it and put it on the card.
- **Opened from:**
  - The update checker creates the form, hidden, if the server INI contains `firmwareurl` (UnitServerInfo: CreateFormUpdateFirmware L133–135).
  - The form's own 1-second timer then polls a queue of detected cards and shows the form itself (`ExecCheckNew`).
  - Cards are queued by `CheckFlashForRegistrator(disk)`. The file manager calls it whenever it searches a **drive root**, i.e. a path of length 3 like `F:\` (UnitFileManager: TFileManager.SearchIn L334–335).
- **Global state:**
  - `registrators[]`: queue of {disk letter, model code, model name, firmware}, protected by a critical section.
  - `registratorModel`: model name, shown in the main window title as ` - <model>`.
  - The `gps` model-code list (registry value `gps`, see 6.12).

**Layout (DFM):** `BorderStyle=bsSingle`, `BorderIcons=[biSystemMenu]`, `BorderWidth=30`, `AutoSize`, design client 300×203. Caption `UpdateFirmware_Title` ("Firmware Update"), backup key `Update_Title` (UnitFormMain: DoTranslate L4726–4728).

| Control | Class | L,T,W,H | Notes |
|---|---|---|---|
| ImageLeft | TImage | 0,0,124,124 | `Form_Update_right.bmp`; after success `Form_Update_ok.bmp` |
| LabelMessage | TLabel | 136,8,164,22 | taCenter, font Vrinda 12pt; message text |
| ProgressBar | TProgressBar | 0,134,241,17 | hidden |
| LabelProgress | TLabel | 0,157 | hidden, `0 Kb (0%)` |
| ButtonYes | TButton | 40,168,88,35 | Default; `Yes` |
| ButtonCancel | TButton | 144,168,88,35 | Cancel (Esc); `UpdateFirmware_BtnNo` "Not now" (backup `Cancel`) |
| TimerCheckModel | TTimer | — | default interval 1000 ms, enabled |

**Card detection — autorun.inf**

`CheckFlashForRegistrator(disk)` works as follows:

1. If the card root already holds any `*.bin` file (a firmware update is pending), stop.
2. Read `X:\autorun.inf` (up to 16 KB) and split it into lines. For each `key=value` (key lower-cased and trimmed, value trimmed):
   - `description` or `label` → model name;
   - `firmware` → firmware version.
3. If both are present, derive the **model code** (the label is upper-cased; checks in this order):

   | Condition | Model code |
   |---|---|
   | label contains `GS800` or `GS-800` | `GS8` |
   | firmware is longer than 3 characters and has the form `G` + digit 1–9 + one of `B G M P U` + `-` | the first 3 characters |
   | first 3 characters are `GS8` | `GS8` |
   | first 3 characters are `G8-` | `G8M` |
   | first 3 characters are `G7-` | `G7M` |
   | first 3 characters are `G6-` | `G6M` |

4. If a code was found:
   - add it to the `gps` format list, which is sent to the server (SF-NET-003);
   - set `registratorModel` to the label;
   - add or replace this disk's entry in the queue.

(UnitFormUpdateFirmware: CheckFlashForRegistrator L67–126)

**Requirements**

- **SF-UPDATEFIRMWARE-001** Timer tick (every 1 s):
  - Do nothing while this form is visible, or while the EXE-update form exists and is visible.
  - Otherwise pop the first entry from the queue. If there was one, pause the timer, run `ExecCheckNew(entry)`, then resume.

  (UnitFormUpdateFirmware: TimerCheckModelTimer L137–160)
- **SF-UPDATEFIRMWARE-002** `ExecCheckNew`:
  1. GET `firmwareURL` with `%model%` replaced by the entry's model code. This is a synchronous request with a 5 s timeout.
  2. Parse the response as `key=value` lines: `version` → server firmware version, `url` → firmware ZIP URL.
  3. If both are present and either (server version > card firmware, as a plain string compare) or (the version strings differ in length), remember the entry and show the form.

  (UnitFormUpdateFirmware: TFormUpdateFirmware.ExecCheckNew L167–188)
- **SF-UPDATEFIRMWARE-003** On show:
  - Load the icon `icon_update.ico` (first time) and the left bitmap; set the form colour to its top-left pixel.
  - Buttons: `Yes`, and `UpdateFirmware_BtnNo` (backup `Cancel`).
  - Message = `UpdateFirmware_NewVer` with `%model%` → model name, `%oldfirmware%` → card firmware, `%newfirmware%` → server version. If that key is missing, use `Update_NewVersion` + `Update_WantDownload ?` instead.
  - Layout: message and image vertically centred against each other. Buttons go 3×progress-height below the taller of the two, centred as a pair. The progress bar spans the full width, 2×its height above the buttons, with the progress label 2 px below it.
  - Focus Yes.

  (UnitFormUpdateFirmware: TFormUpdateFirmware.FormShow L198–231)
- **SF-UPDATEFIRMWARE-004** Closing the form by any route **clears the whole detection queue**. (UnitFormUpdateFirmware: FormCloseQuery L234–236)
- **SF-UPDATEFIRMWARE-005** (trivial) No / Not now closes the form. (UnitFormUpdateFirmware: ButtonCancelClick L239–241)
- **SF-UPDATEFIRMWARE-006** Download: Yes is hidden and Cancel is centred and relabelled `Cancel`. The progress controls are shown, and the firmware URL is downloaded with UI message pumping until done or the form is closed. Progress label `<Kb> Kb   ( <pct>% )`. (UnitFormUpdateFirmware: ButtonYesClick L261–275; onStatus L246–253)
- **SF-UPDATEFIRMWARE-007** If the response is empty, show `Update_Err_Download` ("Downloading failed"; caption `<form caption>. Error`) when the form is still visible, close the form and hide the progress controls. (UnitFormUpdateFirmware: ButtonYesClick L317–323)
- **SF-UPDATEFIRMWARE-008** The response is treated as a **ZIP archive**. The first entry with extension `.bin` and size > 1 MiB is extracted into memory and LabelMessage shows `X:\<entry name>`. If there is no such entry, or it cannot be read, show `Update_Err_Decompress` and close. (UnitFormUpdateFirmware: ButtonYesClick L279–300)
- **SF-UPDATEFIRMWARE-009** Free-space check: the card needs at least 1.1 × firmware size + 1 MiB free. Otherwise show `Mes_NoFreeSpace` and close. (UnitFormUpdateFirmware: ButtonYesClick L301–304)
- **SF-UPDATEFIRMWARE-010** Success:
  1. Write the `.bin` to the card root under its archive name, and write an **empty marker file `X:\firmware.ready`**.
  2. Set the message to `UpdateFirmware_NeedRes` with `%firmware%` → server version (the EN text has no placeholder), and the image to `Form_Update_ok.bmp`.
  3. Relabel Cancel as `Ok` and centre it.

  (UnitFormUpdateFirmware: ButtonYesClick L306–314)
- **SF-UPDATEFIRMWARE-011** Unit init creates the queue lock; finalisation frees it and the queue. (UnitFormUpdateFirmware: initialization L326–327, Finalization L329)

---

### 6.12 Network support: THttpDownloader (UnitHttpDownloader), TAutoUpdateChecker (UnitServerInfo), AddUrlAddons (UnitUtils), UnitBrand

Endpoints and wire formats. These may be dropped in the reimplementation but are documented here.

| Purpose | Request | Response |
|---|---|---|
| Update/ads/firmware discovery | `GET http://update.registratorviewer.com/file/registratorviewer.ini?version=<Self_Version>&langid=<langid>[&gps=<codes>][&did=rw-<id>]` | INI-like text, CR/LF separated `key=value`. Keys are case-insensitive: `version`, `versiontext`, `downloadurl`, `downloadweb`, `adsurl`, `firmwareurl`. |
| EXE update | `GET <downloadurl>` | raw EXE bytes, more than 100 KiB |
| Ads | `<adsurl>` + the same query add-ons, loaded into the main form's embedded browser `WebBrowserAds` | HTML |
| Firmware info | `GET <firmwareurl>` with `%model%` replaced | `version=…` / `url=…` lines |
| Firmware payload | `GET <url>` | ZIP containing a `.bin` larger than 1 MiB |

- **SF-NET-001** HTTP client:
  - Plain HTTP/1.1 over a raw TCP socket. No HTTPS, no proxy and no redirect following.
  - URL parsing: drop `scheme://`, split host from path at the first `/`, optional `:port` (default 80).
  - Request: `GET <path> HTTP/1.1`, `Host: <host>:<port>`, `User-Agent: DATAKAM PLAYER <version> (DATAKAM)`, `Connection: close`, then an empty line.

  (UnitHttpDownloader: THttpDownloader.CreateURL L49–56, Create L60–76, CreateSync L80–90, ClientSocketConnect L115–118)
- **SF-NET-002** Response handling:
  - Headers end at the first blank line. `Content-Length` and `Transfer-Encoding: chunked` are recognised; status codes are **ignored**.
  - The socket is closed once Content-Length bytes have arrived, or once the chunked terminator `0\r\n\r\n` is seen.
  - On disconnect, remaining data is drained. A short Content-Length body counts as failed (empty) and chunked bodies are de-chunked. The response is then marked ready and the ready callback fires.
  - Socket errors are swallowed and the response stays empty.

  (UnitHttpDownloader: ClientSocketRead L125–159, ClientSocketDisconnect L167–195, ClientSocketError L199–201)
- **SF-NET-003** Query add-ons appended to every server URL (with `?` or `&` as needed):
  - `version=<Self_Version>` (major.minor) and `langid=<current langid>`;
  - `gps=<comma list of model codes>` when non-empty (registry value `gps`, see SF-UPDATEFIRMWARE);
  - `did=rw-<id>` when an install ID exists. The ID is 11 random letters created at first run and stored as registry value `id`; a legacy copy under `HKCU\Software\Registrator Viewer\id` is migrated (UnitFormMain: TimerOnStartTimer L5150–5168).

  (UnitUtils: AddUrlAddons L387–398; AddGPSFormat L376–382)
- **SF-NET-004** Progress callbacks fire at most every 0.1 s and once more at the end. `IsReady(timeout)` reports ready when the response is complete, or (if timeout > 0) when the timeout since start has passed. `SimpleGET(url, timeout)` blocks: on the main thread it pumps messages (1 ms sleep), otherwise it sleeps 10 ms per loop. `Cancel` detaches callbacks and closes the socket. (UnitHttpDownloader: ClientSocketRead L146–150, IsReady L204–209, SimpleGET L215–226, Cancel L105–111, Destroy L93–96)
- **SF-NET-005** **Startup update check.** `TFormMain.RestartAutoupdate(ms)` stops and frees any running checker, then starts a new background thread "Auto Update Checker". (UnitFormMain: TFormMain.RestartAutoupdate L3192–3204; UnitServerInfo: TAutoUpdateChecker.Create L40–44, Stop L54–58, Destroy L47–51)
- **SF-NET-006** The checker thread:
  1. Wait `timeoutMS` (polling for termination every 10 ms).
  2. On the main thread, create an HTTP request for the discovery INI (this resets the "gps formats changed" flag).
  3. Wait without timeout until ready or terminated, then parse.
  4. If `downloadurl` is set **and `Self_Version < version`** (plain case-sensitive string compare of e.g. `6.0` vs `6.1`) **and** the user has not declined this session (`descardUpdate`), show TFormUpdateExe.
  5. If `adsurl` is set, navigate the ads browser to it.
  6. If `firmwareurl` is set, create the (hidden) firmware form.

  When absent, `downloadweb` defaults to `downloadurl` and `versiontext` defaults to `version`. (UnitServerInfo: TAutoUpdateChecker.Execute L64–82, InitDownloadQuery L87–89, Parse L97–119, ShowAds L128–129)
- **SF-NET-007** UnitBrand contains no runtime behaviour (unit init/finalisation counters only). Branding is via the hard-coded `DATAKAM PLAYER` / `DATAKAM` literals and the `TRANSLATE_FIX_*` key overrides listed at the top of this section. (UnitBrand: initialization L15)

---

## 7. Open questions

### 7.1 Shell, settings, translation and menus

1. **Single instance.** The original has no single-instance check (SH-001): two windows share the settings and the last write wins. Should the Rust app add single-instance behaviour, for example forwarding command-line files to the running instance? That would be a deliberate deviation.
2. **Command-line switches.** `/whatsnew` and `/del<path>` are also passed to the file manager as file paths (SH-016, SH-018). The Rust app should filter them out. Is `/del` worth keeping at all if the self-updater (§6.10) is dropped?
3. **Telemetry and ads.**
   - At start-up and on map changes, the hidden "analytics" browser navigates to `http://www.registratorviewer.com/googleMapsAPI.html?version=<ver>&langid=<id>` (`ExecAnalytics L1593–1603`).
   - The "ads" browser loads `adsurl` from the update INI and resizes itself to the page's `height` meta value (`WebBrowserAdsProgressChange L5571–5582`).
   - `id` and `gps` are sent to the update server.

   Recommendation: drop all of these. If the update check is kept, make it opt-in.
4. **IE-specific code** (`fixIE`, `IEFix`, the WebBrowser silent flags) means nothing without the IE control. Drop it.
5. **DirectShow-only menu items**: Splitters, Decoders, Renderer, the "(EVRonly)" / "(VMRonly)" postfixes, and the EVR start-up probe (SH-012). The playback/video writer must decide what replaces them. Their **setting names must not be reused** with other meanings.
6. **Translation-loader truncation bug** (TR-006). This spec says not to reproduce it. Please confirm.
7. **Portuguese auto-detection** never fires, because its LANGIDs are written in decimal (TR-009). Fixing this changes visible behaviour on Portuguese systems.
8. **`panel_Browser`** is written with a `.` decimal point but read with the locale-dependent `StrToExtended` (ST-002). On decimal-comma locales the original may fall back or fail. The Rust app should accept both separators.
9. **Settings import.** Should the Rust app import `HKCU\Software\DATAKAM PLAYER\` and the legacy `HKCU\Software\Registrator Viewer\id` on first run? The value names in §3.2 are designed to support this.
10. **"Reset All Settings"** relaunches the executable (SH-024). An in-process reset that rebuilds the UI state would be equivalent. Decide whether a process restart is required.
11. **Help menu.** It shows both "Visit Site DATAKAM" and "Visit Site Registrator Viewer"; the second is the author's site and is probably dead. Keep both?
12. **Window caption during playback** (file details and camera model) belongs to the playback spec. Only the idle caption is fixed here (§1, TR-029).
13. **Menu-icon padding** for items without an icon (TR-022 step 7) is a Windows-menu quirk and is not required in egui. Confirm that it can be dropped.

### 7.2 Forms 6.4 to 6.7 (Copy, Cropping, Cropping item, File property)

1. SF-CROPPING-023(a): when the output name is empty, the message text is Translate(`Repair_FullOutputFile`) = "Result" with caption "Error" — looks like a copy/paste from TFormRepair. Reproduce literally or show a meaningful message?
2. SF-CROPPING-011: a frame-size mismatch only warns; the mismatching item stays in the list and is later rejected by the source group ("Different format …", SF-CROPPING-025). Keep or reject immediately?
3. SF-CROPPING-027: subtitle offsets use the length of the combined GPS track; if "Meta Info" is unchecked but ".srt" is checked, that track is never extended, so every item's subtitles start at offset 0. Original bug — reproduce or fix?
4. SF-CROPPING-025: "Cannot open file " / "Different format " and SF-CROPPING-035 "Fragment of " are hard-coded English (not translatable).
5. The exact semantics of the source-group "speedUpFPS" vs key-frame copy and of the embedded "fragment" GPS blob format (gpsSaverFragment) live in UnitSources / UnitGpsManager — need to be specified by the video/GPS writers for the export to be reimplemented.
6. SF-CROPPING-024: the main window is hidden during export and the form deletes the partial output only when cancelled via hiding; an exception in "Copying frames" leaves a partial file. Keep?
7. SF-COPY-010 blocks the UI thread for 3 s after opening Explorer; SF-COPY-006 never resumes partial copies (always rewrites from 0) and SF-COPY-011 leaves a partial destination file on cancel. Reproduce?
8. SF-FILEPROPERTY-006 hidden debug row (mouse at X = 0) — drop in the reimplementation?
9. SF-FILEPROPERTY-006: the exact meaning of the "(position)" prefix fields inside TBlockDateTime could not be fully resolved from the record layout (symbols give only anonymous sub-records); described from behaviour.
10. TFormCroppingItem OK button text "Ok" is never translated (DFM text).

### 7.3 Forms 6.1 to 6.3 and 6.8 to 6.12 (About, What's New, message box, Repair, Setup device, Updates, Network)

1. **ShowMessage clipboard header:** the exact separators between version, `Win<maj>.<min>` and the date-time are inferred from the string pool (`DATAKAM PLAYER `, `, `, `Win`, `.`, CRLF). The concatenation order is not fully proven.
2. **Repair: duration state value 2** is taken to mean "file could not be opened, presumed broken". The enum definition was not recovered; values 1 = approximate (`~` prefix) and 3 = exact are confirmed.
3. **Repair reference auto-pick:** the time fields compared are `time.fs[0..2]`, assumed to be start, end and duration. The exact meaning, and the unit of the 999 999 threshold, should be checked against the file-list spec.
4. **Firmware model-code mapping:** the decompiler does not show where the record's model code, model name and firmware fields are assigned; the semantics are inferred from how ExecCheckNew uses them. Also, when the label is missing the code is used as the label, yet a label is required earlier, so that branch is dead code.
5. **Version compares are plain string compares** (EXE: `Self_Version` "major.minor" vs INI `version`; firmware: card vs server version string). For example `6.10 < 6.9`. Decide whether to keep this or switch to numeric compare if the feature is kept.
6. **Update/ads/firmware endpoints** are plain-HTTP and on a domain that is probably dead. They also send a persistent install ID (`did=rw-…`). Recommend dropping or making it opt-in. The EXE self-replace flow (rename the running EXE, `/whatsnew "/del…"`) is Windows-specific.
7. **SetupDevice default blob** bytes 0–5 and 123–127 (`0E 01 52 50 F1 01` … `00 50 0E 05 53`) are opaque; their meaning (header/magic/checksum?) is unknown. The app neither validates nor recomputes them.
8. **Translations of the frame-size items:** the frame-size items and the X/Y/Z captions are not translated (fixed DFM text). Only the Custom item uses `SetupDevice_OtherSize`.
