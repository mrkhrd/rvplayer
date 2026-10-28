# Registrator Viewer 6.0.0.22: File List and File Management (main window)

Source of truth for reimplementing the file list area in rvplayer. Everything here was reconstructed from the decompiled Delphi units, the original source line numbers (`Lnnn`), the DFM forms and the unpacked binary (constants and string literals were recovered from the disassembly).

Citation format: `Unit: Class.Method Lnnn–Lmmm`. Abbreviations: **FM** = `TFileManager` (UnitFileManager), **Main** = `TFormMain` (UnitFormMain).

---

## 1. Overview

The file list sits in the right-hand panel of the main window, under the map. It shows one row per **item**. An item is either a single dashcam video file or a **group** of consecutive segment files that play as one timeline.

- The list is **virtual** (owner-data). Rows are produced on demand from the file manager (`TFileManager`), a background thread that does the following:
  - finds video files and GPS logs (auto-search of the current folder, the exe folder, the `DCIM` folder and USB flash drives, or explicitly added files and folders);
  - reads each file's real duration, resolution and embedded GPS ("preview");
  - matches GPS sidecar logs to videos;
  - groups consecutive segments automatically;
  - keeps everything sorted.
- While it works, row 0 shows a progress text. The list is fully owner-drawn: grid, selection colours, auto-shrinking font, "`..`" truncation, and four status icons (lock, partial lock, video warning, GPS found).
- A row of flat icon buttons (`PanelFileListMenu`) above the list mirrors the right-click popup menu `PopupMenuListView`.
- Commands cover:
  - adding, removing and refreshing files, and deleting them from disk;
  - setting and clearing the read-only "lock";
  - manual grouping and ungrouping;
  - track export, Google Earth, attaching a track, and shifting video time against GPS;
  - showing a file in Explorer and opening a properties window.
- Selecting a single row loads it as the current item: it is prepared for playback and its GPS route is drawn on the map. Double-click or Enter plays it.

---

## 2. UI elements

### 2.1 Containers (TFORMMAIN.dfm)

| Control | Type | Parent | Align | DFM size | Notes |
|---|---|---|---|---|---|
| `PanelRight2` | TPanel | PanelRight | alClient | 510×410 | Holds the map, the splitter and the list. |
| `WebBrowser` | TWebBrowser | PanelRight2 | DFM alBottom; set to **alTop** at runtime when the list is visible | h=234 | The map. |
| `SplitterListView` | TSplitter | PanelRight2 | alBottom in the DFM; runtime alignment = the map's alignment | h=7 | Visible only when the list and the map are both visible. |
| `PanelListView` | TPanel | PanelRight2 | alClient (runtime 5 = alClient) | 510×155, bevel none | Hidden when View → File List is unchecked. |
| `PanelFileListMenu` | TPanel | PanelListView | alTop | height **20**, bevel none | Toolbar; its buttons are created at runtime (§2.5). |
| `ListView` | TListView | PanelListView | alClient | 510×135 | See §2.2. |

Visibility and layout come from `Main: N_View_FileListClick L3049–L3090`:
- `PanelListView.Visible` follows the checked state of `N_View_FileList`.
- `SplitterListView` is visible only if the list is visible and the map browser is visible.
- If the list is visible: the map is aligned top (given height 10 if it was 0), the splitter is placed right under the map with the map's alignment, and the list is alClient.
- If the list is hidden: the map becomes alClient (when shown) and the list and splitter are un-aligned.

### 2.2 ListView properties

From the DFM:
- ViewStyle = vsReport; **OwnerData** = True; **OwnerDraw** = True; **MultiSelect** = True; RowSelect = True; ReadOnly = True.
- GridLines = True; HideSelection = False; FullDrag = True (header columns can be reordered by dragging).
- PopupMenu = PopupMenuListView; SmallImages = ImageListView (16×16).
- Events: OnClick, OnColumnClick, OnColumnDragged, OnCustomDraw, OnData, OnDblClick, OnKeyUp, OnSelectItem.

Runtime setup:
- DoubleBuffered = True. The window procedure is subclassed to catch header end-of-resize (§FL-024). See `Main: FormCreate L939–L940`.
- Background colour = translation colour key `colorListBG` (EN: `clWindow`), per `Main: FormCreate L945`.

### 2.3 Columns

"ID" is the column's identity. Persistence, sorting and data all use the ID, never the visual position.

| ID | DFM caption | Caption key → EN text | Align | DFM width | Width after FormCreate (= "default") | Content |
|---|---|---|---|---|---|---|
| 0 | Name | `FileList_Col_Name` → "%Name%" → **Name** | left | 113 | 113+36−13 = **136** | Display name (`fileFace`; for groups, the composed name). Lock icon. |
| 1 | Duration | `FileList_Col_Duration` → **Time** | right | 45 | **47** | Duration string. Video-warning icon. |
| 2 | Date and Time | `FileList_Col_DateTime` → **Date, time** | right | 95 | **97** | Start date and time. |
| 3 | km | `FileList_Col_Track` → **Trip** | right | 54 | **56** | Track length with unit. GPS-found icon. |
| 4 | km/h | `KmetersPerH` → **km/h**, or `MilesPerH` → **mph** when British units are on | right | 38 | **40** | Average speed (number only). |
| 5 | File Location | `FileList_Col_FilePath` → **File Location** | left | 36 | **1** (effectively hidden) | Full path(s); for groups joined with ", ". |
| 6 | Mb | `Mb` → **Mb** | right | 43 | **45** | Size in MiB. |
| 7 | Resolution | `FileList_Col_PictSize` → **Resolution** | right | 62 | **64** | `WxH`. |

Sources:
- Widths: `Main: FormCreate L929–L936`.
- Captions: `Main: DoTranslate L4674–L4685`. The caption array is indexed by column ID, so captions follow columns that have been moved.

### 2.4 Image list `ImageListView`

The images are loaded in this order, so the index is fixed (`Main: FormCreate L885–L888`). Each comes from an RCDATA 16×16 24-bit BMP; the transparent colour is the bottom-left pixel (`Main: BuildMask L742–L754`, `LoadImageListIcon L758–L763`).

| Index | Resource | Meaning |
|---|---|---|
| 0 | `FileList_icon_key.bmp` | Item fully locked (all files read-only). |
| 1 | `FileList_icon_key_partial.bmp` | Group partly locked. |
| 2 | `FileList_icon_video_warning.bmp` | The internal parser could not read the duration (`durationState = 2`). |
| 3 | `FileList_icon_gps_found.bmp` | Valid GPS track present (`info.gps = 2`). |

### 2.5 Popup menu `PopupMenuListView` and toolbar

Menu order is as in the DFM. Shortcuts are active while the list has focus.
- Icons come from RCDATA `<ItemName>.bmp`, e.g. `N_FILELIST_ADD.BMP` (`Main: LoadMenuBitmaps L767–L774`).
- `N_FileList_Save` reuses `N_File_Save_Video`'s bitmap, and `N_FileList_Repair` reuses `N_File_Repair_Video`'s (`Main: FormCreate L891–L892`).
- Captions are the translation key equal to the item name (`Main: TranslateMainMenuItem L4535–L4570`).

| # | Item | EN caption | Shortcut | Toolbar button | Handler |
|---|---|---|---|---|---|
| 1 | `N_FileList_Open` | Open | Enter | **no** | `ListViewDblClick` |
| – | `N_SFL1` | — | | **no** (no bevel) | |
| 2 | `N_FileList_Add` | Add Files | Insert | yes | `N_File_OpenClick` |
| 3 | `N_FileList_Del` | Remove from List | Delete | yes | `N_FileList_DelClick` |
| 4 | `N_FileList_Refresh` | Refresh File List | F5 | yes | `N_FileList_RefreshClick` |
| 5 | `N_FileList_Clear` | Clear File List | Ctrl+N | yes | `N_FileList_ClearClick` |
| – | `N_SFL2` | — | | separator bevel | |
| 6 | `N_FileList_Copy` | Copy File to Computer | Shift+C | yes | `N_FileList_CopyClick` |
| 7 | `N_FileList_DelDisk` | Delete File from Disk | Shift+Del | yes | `N_FileList_DelDiskClick` |
| 8 | `N_FileList_DelDiskAll` | Delete All Unlocked Files from Disk | — | **no** | `N_FileList_DelDiskAllClick` |
| 9 | `N_FileList_Save` | Save whole group | — | yes | `N_FileList_SaveClick` |
| 10 | `N_FileList_Repair` | "%N_File_Repair_Video%" → Repair broken file | — | yes | `N_FileList_RepairClick` |
| – | `N_SFL3` | — | | bevel | |
| 11 | `N_FileList_SetLock` | Set Lock (Protect from Deletion) | — | yes | `N_FileList_SetLockClick` |
| 12 | `N_FileList_UnsetLock` | Unlock | — | yes | `N_FileList_SetLockClick` |
| – | `N_SFL4` | — | | bevel | |
| 13 | `N_FileList_Group_With` | "Group with" + `" . . ."` appended at translate time | — | yes | `N_FileList_Group_WithClick` |
| 14 | `N_FileList_Ungroup` | Ungroup | — | yes | `N_FileList_UngroupClick` |
| 15 | `N_FileList_Group_Auto` | Find Groups Automatically | — | yes | `N_FileList_Group_AutoClick` |
| 16 | `N_FileList_Ungroup_All` | Ungroup All | — | yes | `N_FileList_Ungroup_AllClick` |
| – | `N_SFL5` | — | | bevel | |
| 17 | `N_FileList_Track_Google` | Open Track in Google Earth | — | yes | `N_FileList_Track_GoogleClick` |
| 18 | `N_FileList_Track_Export` | Export Track | — | yes | `N_FileList_Track_ExportClick` |
| 19 | `N_FileList_Track_Set` | Attach Track | — | yes | `N_FileList_Track_SetClick` |
| 20 | `N_FileList_Track_GPSLog` | Show Related GPS Log | — | yes, **width 0** (Visible=False) | `N_FileList_Track_GPSLogClick` |
| 21 | `N_FileList_Track_Move` | Move video position | — | yes | `N_FileList_Track_MoveClick` |
| – | `N_SFL6` | — | | bevel | |
| 22 | `N_FileList_ShowExplorer` | Show File in Explorer | — | yes | `N_FileList_ShowExplorerClick` |
| 23 | `N_FileList_Property` | Property | — | yes | `N_FileList_PropertyClick` |

Sources: `Main: TranslateMainMenuItem L4545–L4546` (the " . . ." suffix); `UnitFormMain` DFM.

**Toolbar construction** (`Main: CreateFileListMenuButtons L805–L857`). The menu items are walked in order, skipping `N_FileList_DelDiskAll`, `N_FileList_Open` and `N_SFL1`. A running x position starts at 0.
- **Separator** → a TBevel with shape "left line": top 2, left x+1, width 2, height 16 (panel height − 4). x advances by 4.
- **Item** → a flat TSpeedButton:
  - top 0, left x;
  - width = 20 (the panel height), or **0** if the item is invisible; height 20;
  - x advances by the button width;
  - Tag = menu index;
  - two glyphs: the original bitmap and a generated "disabled" version (`MakeDisabledImage`, factor 2.0);
  - ShowHint on.
- Hint = the menu item caption, refreshed on every translation (`Main: DoTranslate L4671–L4672`).
- **Click** → runs the menu item's OnClick, then re-evaluates enabling (`Main: OnFileListMenuButtonClick L5592–L5599`).
- **Enabled state** mirrors the menu item and is copied at the end of every enabling pass (`Main: PopupMenuListViewPopup L3563–L3564`).
- The popup menu is not owner-drawn (`Main: DoTranslate L4631–L4633`).

### 2.6 Main menu items in this area

| Item | Menu | EN caption | Shortcut | Handler |
|---|---|---|---|---|
| `N_File_Open` | File | Open | Ctrl+O | `N_File_OpenClick` (same as Add Files) |
| `N_FileList_Refresh2` | File | Refresh File List (key resolved by stripping the trailing digit) | Ctrl+R | `N_FileList_RefreshClick` |
| `N_FileList_Clear2` | File | Clear File List | Ctrl+N | `N_FileList_ClearClick` |
| `N_View_FileList` | View | File List | F2 | `N_View_FileListClick` (checkable, default checked, persisted) |
| `N_View_Movie_Overlap` | View → Timeline | Set video files overlap time | Ctrl+J | `N_View_Movie_OverlapClick` |

The "strip the trailing digit" rule for caption keys is in `Main: TranslateMainMenuItem L4540–L4543`.

### 2.7 Dialogs

- **`OpenDialogFileAdder`** (TOpenDialog):
  - Options: HideReadOnly, AllowMultiSelect, EnableSizing.
  - Filter (3 entries):
    1. `DashCam Files (*.mov, *.mp4)` = `*.mov;*.mp4;*.mp4v`
    2. `Video Files` = 3GP ASF AVI AVS CME COF CSA DIVX FLV M1V M2V MLS MOV MP4 MPE MPEG MPG OGM MKV XFL RM RMVB VIV VOB WMV XQT VP3–VP7
    3. `Any Files` = `*.*`
- **`OpenDialogTrack`** (Attach Track):
  - At runtime the filter is replaced by the GPS manager's opener filter: "All files", then "All supported formats (…)", then one entry per extension. FilterIndex = 2 (`Main: N_FileList_Track_SetClick L5232–L5233`).
- **`SaveDialogGpsConverter`** (Export Track):
  - Options: OverwritePrompt, HideReadOnly, EnableSizing.
  - Filter built from the export formats (§FL-071).
- **`TFormGroup`** ("Grouping"):
  - Layout: modal, 713×621; ListView alClient (checkboxes, multi-select); bottom panel 41 px with **Group** (`Grouping_Btn`) at (8,8) 113×25 and **Cancel** right-aligned; TimerDrawRoute 500 ms.
  - Caption `Grouping_Title` = "Grouping"; positioned at main form left+10, top+50.
  - Source: `UnitFormGroup: TFormGroup.Popup L56–L89`.
- **`TFormFileProperty`** ("Property"):
  - Non-modal, one new window per selected row; 441×494.
  - StringGrid with 2 columns, alClient; bottom panel 41 px with **Copy All to Clipboard** (`Property_CopyAll`) 217×25 at (8,8).
  - Source: `UnitFormFileProperty: TFormFileProperty.Popup L143–L165`.
- **`TFormSelectFolder`**: used **only** by Copy File to Computer for the destination, not for adding files. Detailed in the Copy spec; out of scope here.

---

## 3. Requirements

Numbers are grouped by topic (A = FL-001–012, B = 020–021, C = 022–025, D = 030–037, E = 040–047, F = 050–090), so there are gaps between groups.

### A. File manager lifecycle and population

**FL-001 Manager creation at startup.**
- On the first start-up timer tick, the command-line parameters are collected (`ParamStr(1..ParamCount)`).
- If there is at least one, a manager is created **without** auto-search:
  - GPS is allowed unless View → Profile → "My dash cam can record only video" (`N_View_Profile_Video`) is checked;
  - every parameter is passed to Add (FL-005);
  - the app waits up to 10 s (processing messages) for the background preview to go idle;
  - the list is repainted, and if it has rows, row 0 is selected and played.
- If no manager was created, or it holds 0 items, Refresh (FL-002) is executed instead.

Sources: `Main: TimerOnStartTimer L5170–L5186`; `FM: WaitForPreview L2374–L2384`.

**FL-002 Refresh File List** (F5, Ctrl+R, toolbar).
- Any existing manager is stopped (thread terminated and awaited) and destroyed. **All items are discarded, including manually added ones.**
- A new manager is created **with auto-search** (FL-003); GPS is allowed as in FL-001.
- Unit strings are refreshed (FL-090).
- Sort column = registry `col_sort` (default **2**, Date); ascending = registry `col_sort_dur` (default **true**).

Sources: `Main: N_FileList_RefreshClick L4448–L4459`; `FM: Create L197–L211`.

**FL-003 Auto-search locations.** When auto-search is on, the background thread first queues these folders, in this order (`FM: Search L293–L307`):
1. the current working directory;
2. the folder of the executable;
3. if the current directory is not a UNC path (does not start with `\\`): `<drive of current dir>:\DCIM\`;
4. `X:\` for every drive letter A–Z that is a **valid USB flash drive** — removable type, USB bus, ready, non-zero size (`UnitUtilsFile: IsValidUSBFlashDrive L708–L715`).

Rules:
- A folder is skipped if it is already queued (case-insensitive) or if its path **contains the user's Desktop folder path** (`FM: Add (nested) L283–L292`).
- The status text is "Searching" (key `Searching`).

**FL-004 Folder scan rules** (`FM: SearchIn L315–L392`; one queued folder is processed per background step: `FM: DoTaskAction` action 1).
- Recursion depth is limited to 5 levels (the queued folder is level 0; deeper than level 4 is ignored). The scan aborts when the thread is terminated or the queue has been emptied (by Clear).
- When scanning a drive root (path length 3), the firmware-update helper `CheckFlashForRegistrator` is called. Out of scope.
- Entries are skipped if they are:
  - Hidden or System;
  - named exactly (case-sensitive) `.`, `..`, `nandlog`, `System`, `Windows`, `Program Files`, `ProgramData` or `Users`.
- Sub-folders are recursed into.
- A file whose lower-cased extension is a known GPS log extension (§4.5) is added to the **GPS log pool** if all of these hold: GPS is allowed, the file is smaller than 10 MiB (10 485 760 bytes), and it is not already in the pool (compared by lower-cased path). It records name, lower-cased name, creation time, last-write time and size.
- A file with extension `.mov`, `.mp4` or `.mp4v` that is not already in the list is added as a new single-file item (§4.2). **No other video extensions are found by scanning.**
- The status text is "Searching - <folder>". The list is notified at most every 0.3 s while scanning, and once more at the end if anything was added.
- After the whole folder is done:
  - the folder is removed from the queue (the last queued folder takes its slot);
  - fast grouping (§4.6.1) runs;
  - items are sorted, the GPS log pool is sorted by lower-cased name, the status is cleared, and the list is notified (state −3).

**FL-005 Add (programmatic)** (`FM: Add L1232–L1237`, `FM: AddFile L1243–L1270`, `FM: AddDirectory L1273–L1282`).
- **Folder path** → appended to the search queue (scanned later as in FL-004).
- **Existing file**:
  - If its extension is a GPS log extension, the GPS file itself is **not** added. Instead, the app tries to add `<same path and name>.mov` and `<same path and name>.mp4`.
  - Otherwise, if the file is already in the list (case-insensitive full path), that file's preview is reset: video "previewed" is cleared, and its embedded/sidecar GPS info and GPS log are emptied. It is then re-analysed by the background thread.
  - Otherwise it is added as a new single-file item (§4.2), **whatever its extension**.
- **Neither a file nor a folder** → ignored silently.

**FL-006 File → Open / Add Files** (Ctrl+O, Insert, toolbar).
- Shows `OpenDialogFileAdder`. On OK, every selected path is passed to AddFiles (FL-008).
- Sources: `Main: N_File_OpenClick L1542–L1549`.

**FL-007 Drag and drop.**
- The main form accepts dropped files (`DragAcceptFiles`) anywhere on the window (`Main: FormCreate L867`).
- On drop, every dropped path is collected. Paths of 255 characters or more are left empty and therefore ignored. The list is passed to AddFiles.
- Folders are allowed; they are queued for scanning.
- Sources: `Main: WMDropFiles L5047–L5063`.

**FL-008 AddFiles.**
- The cursor becomes an hourglass. Each path is passed to Add (FL-005).
- If the form is visible and the list can take focus, the list is focused. The cursor is restored.
- Sources: `Main: AddFiles L5068–L5079`.

**FL-009 Clear File List** (Ctrl+N, toolbar).
- Removes all items, empties the GPS log pool and the search queue. A running scan therefore stops at its next check.
- Playback is **not** stopped explicitly.
- The original only updates the row count on the next manager notification (see OQ-2). The reimplementation must refresh the list immediately.
- Sources: `Main: N_FileList_ClearClick L5267–L5269`; `FM: Clear L2343–L2353`.

**FL-010 Background processing order** (`FM: Execute L234–L270`, `FM: DoTaskAction L491–L679`).
- The thread runs at lower priority.
- On start:
  - status "Initializing" (key `Initializing`), then a 200 ms pause;
  - the auto-search queueing (FL-003) if enabled;
  - the directory-listing cache is cleared.
- Then it loops. Each iteration performs at most one task, in this priority order:
  1. **Scan** the first queued folder (FL-004).
  2. **Preview a video** that has not been previewed yet (§4.4.1). This is the first such file found scanning items from last to first. Status "Preview NN%" covers 0–40 % when GPS is allowed, else 0–99 %, by the fraction of previewed videos.
  3. **Preview a GPS log** from the pool that has not been previewed yet (§4.4.2). The log with the shortest creation-to-write span goes first. Status 40–80 %.
  4. **Preview the GPS info of a video file** (embedded or sidecar, §4.4.3) not yet done. Status 80–100 %.
- After task 2 or 4 the result is written back to the matching file record (found again by path if items moved), and the item is recomputed (§4.7).
  - If it was the last unpreviewed video: fast grouping and sort.
  - If it was the last unpreviewed GPS info: fast grouping.
  - The list is notified.
- When an iteration finds no task:
  - If anything changed since the last idle: status "Grouping" (key `Grouping_Title`), notify (−1), **automatic grouping** (§4.6.2), clear the status, notify (−2), clear the directory cache.
  - The manager is then marked idle and sleeps 500 ms.
- Exceptions inside a preview are swallowed. A video that fails to load additionally shows the message "Failed load video file <path>" (`FM: PreviewItemVideo` exception handler).

**FL-011 Status row.**
- While the manager has a non-empty status text:
  - its count is at least 1, even with no items (`FM: Count L1285–L1294`);
  - **row 0 is rendered as the status row** (`FM: PrintItem L1319–L1324`):
    - Name = item 0's name, if any items exist;
    - the only sub-item = `". . . " + status + " . . ."`;
    - no icon.
- The status text is drawn left-aligned from the start of column position 1 to the right edge of the row, spanning the remaining columns (`Main: ListViewCustomDraw L3783–L3786`, L3854–L3857).
- Item 0's real data is hidden while the status is shown.

**FL-012 List synchronisation** (`Main: OnFileManagerChanges L3381–L3414`). This runs on every manager notification, in the UI thread.
- If the row count differs from the manager's Count, the count is updated and the enabling rules are re-run (FL-040).
- The list is repainted.
- If a "last selected name" is remembered and the current row's Name differs from it:
  - search all rows for the first whose Name equals it;
  - if found: clear the selection, select and focus that row;
  - otherwise: clear the selection, set no current row, and forget the name.
- Notification −2 (after auto-grouping) also restarts the auto-update check when the GPS formats changed. Out of scope.

### B. Row content and drawing

**FL-020 Row data** (`Main: ListViewData L3891–L3893` → `FM: PrintItem L1327–L1346`).

| Column ID | Text |
|---|---|
| 0 | Name = group display name (§4.7). |
| 1 | Duration string (§4.8). |
| 2 | Date-and-time string. |
| 3 | Track-length string. |
| 4 | Average-speed string. |
| 5 | Full path(s), joined with ", " for groups. |
| 6 | Size string. |
| 7 | Resolution string. |

Icons:
- Name column: lock state 1 → icon 0 (key); lock state 2 → icon 1 (partial key); otherwise none.
- Duration column: icon 2 if `durationState = 2`.
- Trip column: icon 3 if the item's GPS state is 2 (valid GPS fix).

**FL-021 Custom drawing** (`Main: ListViewCustomDraw L3756–L3888`). The whole list is drawn by the application; default drawing is off.
- **Font:** name = translation `LangFont` (default **Arial**), height 14 px.
- **Cell layout:** cell rectangles come from the cumulative widths of columns in *display* order. Cell left edge = column start − 1; top = row top − 1.
- **Selected row:** the whole row is filled with `colorListSelBG` (EN `clHighlight`) and outlined in `colorListGrid` (EN `#EEEEEE`).
- **Unselected cell:** filled with the list background (`colorListBG`) and outlined in `colorListGrid`.
- **Narrow columns:** if a cell is narrower than 8 px, nothing is drawn in it. For unselected rows, a 1-px vertical line in `colorListBG` hides the grid. This is how the 1-px File Location column disappears.
- **Icons:** drawn at (cell left + 1, top + 1). In the Trip column the text offset is reduced by 3 px.
  - If the cell has an icon, or it is the Name column and **any** item in the list is locked (`FM: HasAnyImageIndex0 L1356–L1368`), the text starts 17 px further right.
  - So once any item is locked, all names shift right.
- **Text area:** the cell shrunk by 1 px on each side.
- **Font shrinking:** except for File Location (ID 5), while text width ≥ cell width − 1 and font height ≥ 11, the font height drops by 2 (14 → 12 → 10).
- **Truncation:** File Location always gets truncation; other columns only when the text still does not fit (width ≥ cell width).
  - Start with x = width of "..". For each character, add its width. At the first character where x exceeds the cell width, the text becomes the preceding characters + `..`.
- **Alignment:** right-aligned columns place the text at right − text width − 2 (the status text is always left-aligned); left-aligned columns at cell left + 1. Text is centred vertically.
- **Text colour:** `colorListSelText` (EN `clHighlightText`) on selected rows, else `colorListText` (EN `clWindowText`).
- **Focus rectangle:** the focused row (or, if none, the current row index) gets a dotted 1-px rectangle in `colorListText` with no fill, drawn over the row.
- **Visible range:** only rows intersecting the update rectangle are drawn. Only the first min(sub-item count, column count − 1) + 1 display positions are drawn per row.

### C. Columns: widths, order and sorting

**FL-022 Default widths:** see the table in §2.3. The adjusted widths are remembered as the per-column defaults (`Main: RestoreFileListColumns L707–L712`).

**FL-023 Restore at startup** (`Main: RestoreFileListColumns L705–L731`).
- For each column ID i:
  - registry `col<i>_w`, if ≥ 0: if it equals the default, the value is deleted; otherwise it becomes the width.
- Then for each display position p:
  - registry `col<p>_i` (default p): if 0 ≤ value < 8, the column with that ID is moved to position p.

**FL-024 Save on resize or reorder** (`Main: ListViewColumnDragged L5274–L5290`).
- Triggered by the OnColumnDragged event and by header HDN_ENDTRACK A/W notifications, i.e. the end of a column resize (`Main: NewListViewProc L5298–L5319`). Only acts while the form is visible.
- For every display position p, with c = the column at p:
  - width w = max(1, current width); write `col<c.ID>_w` = w, or delete it if w equals the default;
  - write `col<p>_i` = c.ID, or delete it if c.ID = p.

**FL-025 Sorting by header click** (`Main: ListViewColumnClick L3417–L3421`; `FM: SetSortColumn L1514–L1522`, `FM: Compare L1371–L1389`).
- Clicking a column header sorts by that column's ID. Clicking the current sort column toggles the direction; clicking another column sorts it ascending.
- The new state is saved to `col_sort` (ID) and `col_sort_dur` (true = ascending).
- No header arrow is shown.
- The manager stays sorted by this key: after scans, groupings, removals and preview completion.
- Sort keys per column ID:

| ID | Key | Comparison |
|---|---|---|
| 0 | Display name | Upper-cased, ordinal |
| 1 | Total duration in ms | |
| 2 | **Start time** (finish − duration) | |
| 3 | Track length | |
| 4 | Average speed | |
| 5 | Combined full path | Case-sensitive, ordinal |
| 6 | Total size | |
| 7 | Width + height | |

- Descending simply negates the comparison. The algorithm is a non-stable quicksort.
- After a header-click sort, no code re-selects the previously selected item, so the selection stays on the same row *index*. See OQ-3; the reimplementation should keep the selection on the same item.

### D. Selection, activation, keyboard

**FL-030 Ctrl/Shift tracking.** Global key-down and key-up messages for VK_CONTROL and VK_SHIFT maintain the two flags `Control_pressed` and `Shift_pressed` (`Main: ApplicationEventsMessage L4394–L4406`).

**FL-031 Selection change** (`Main: ListViewSelectItem L2562–L2628`).
- Let n = number of selected rows. Let idx = −1 if n ≥ 2, else the current row index.
- **If idx < 0:** a deferred message is posted. When it is handled (`Main: OnWmUser3 L2631–L2637`): if the selected count ≠ 1, the remembered name is forgotten and playback is stopped (the selection is *not* cleared). The enabling rules then run.
- **Otherwise:** the remembered name := Name of row idx.
  - If the event is a *select* (not a deselect), and either row idx is not the item now playing (FL-034) or Play is disabled, then:
    - stop playback; show the app-start cursor;
    - load item idx as the **current item** (current file index 0);
    - enable the Play button and the play-speed panel;
    - build one GPS movement record per file:
      - use the file's GPS **info** (embedded or sidecar) if present, else its **GPS log**, else an empty record;
      - logs are loaded once per selection change and shared between files that use the same log path;
      - a loaded record is cloned, cropped to the file's time window, and its statistics are written back into the item (`FM: PreviewAgainAndCrop L2253–L2269`);
      - if a file has both info and a log, altitude is taken from the log;
    - expand the records into one timeline; set up the zoom bar; re-layout;
    - draw the route on the map (map spec), refresh the position display;
    - set the speed-button presets from the first file's GOP size (playback spec);
    - restore the cursor.
  - The enabling rules (FL-040) run in all cases.
- Source for the GPS log loading: `Main: GetGPSLog L2544–L2561`.

**FL-032 Single click** (`Main: ListViewClick L2640–L2648`).
- If Play is enabled (an item is loaded), the player is not active, and neither Ctrl nor Shift is held: Play is set "up" and the play action runs. The current item is opened paused at its last position, so its frame is shown.
- If Ctrl or Shift is held: the selection logic of FL-031 is re-evaluated. Selecting several rows therefore stops playback.

**FL-033 Key up** (`Main: ListViewKeyUp L2651–L2654`). If Ctrl or Shift is held, the selection logic is re-evaluated as in FL-032.

**FL-034 "Now playing" test** (`Main: IsNowPlaying L2421–L2426`). Row i is "now playing" when its combined path string equals that of the current item.

**FL-035 Double-click / Enter / Open** (`Main: ListViewDblClick L2657–L2665`).
- If Play is disabled: stop, then load the current row (FL-031 as a select).
- Then Play is set "down" and the play action runs, so the item plays.

**FL-036 Select all.** Ctrl+A selects all rows, but only while the main window or the full-screen window is active (`Main: ApplicationEventsShortCut L4437–L4438`).

**FL-037 Play next.** When Play Next is enabled and the current item reaches 100 %, the next row is selected and played; after the last row it wraps to row 0 (`Main: TimerTimer L1259–L1275`; playback spec).

### E. Menu and toolbar enabling (`Main: PopupMenuListViewPopup L3519–L3565`)

The rules run when the popup opens, and also after selection changes (FL-031), list count changes (FL-012), toolbar clicks and startup. Let **R** = "a current row exists" (row index ≥ 0) and **S** = the set of selected rows.

**FL-040** All menu items are enabled if R, else disabled.

**FL-041** Add Files, Delete All Unlocked Files from Disk, Refresh and Clear are **always enabled**.

**FL-042** If R, these items start disabled: Set Lock, Unlock, Show Related GPS Log, Ungroup and Export Track. Each one is then enabled if *any* selected item satisfies its condition:
- Set Lock: the item's lock state ≠ 1, i.e. not fully locked.
- Unlock: lock state ≠ 0.
- Ungroup: the item has more than one file.
- Show Related GPS Log: some file of the item has a GPS log.
- Export Track: the item's GPS info path is non-empty.

**FL-043** If R:
- Open Track in Google Earth = Export Track's state.
- Delete File from Disk is enabled **only if Unlock is disabled**, i.e. no selected item is (partly) locked.

**FL-044** Open is enabled only when exactly one row is selected. Attach Track and Move video position follow Open.

**FL-045** Group with…, Find Groups Automatically and Ungroup All are enabled when the list has at least one row. In practice, row 0 has at least one sub-item.

**FL-046** Copy, Remove from List, Save whole group, Repair, Show in Explorer and Property are enabled whenever R holds. They have no further checks.

**FL-047** Each toolbar button's Enabled equals its menu item's Enabled.

### F. Commands

**FL-050 Remove from List** (Delete) (`Main: N_FileList_DelClick L3494–L3500`; `FM: Remove L1563–L1576`).
- Every selected row, from last to first, is removed from the manager (matched by combined path). Files on disk are untouched.
- After each removal the list is re-sorted. Then the selection is cleared and the current row is set to none.
- The original updates the row count only on the next notification (OQ-2).

**FL-051 Delete File from Disk** (Shift+Del) (`Main: N_FileList_DelDiskClick L3428–L3454`).
- For each selected row: if it is the item now playing, playback is stopped. The item is collected.
- The message lists each item's combined path, one per line.
- A confirmation dialog appears: Confirmation icon, buttons Yes and No. Text:
  `Mes_SureToDelFile` + CRLF + the list + `" ?"` + CRLF + CRLF + `Mes_NoWayToBack` + `" ! ! !"`.
  EN: "Are you sure to full delete file", the files, " ?", "There will NO way to recover files ! ! !".
- On **Yes**, each collected item is deleted (FL-053).
- In every case the selection is then cleared and the current row set to none.
- The command is unavailable when any selected item is (partly) locked (FL-043).

**FL-052 Delete All Unlocked Files from Disk** (`Main: N_FileList_DelDiskAllClick L3462–L3489`).
- Playback is stopped first.
- All items whose lock state is 0 are collected. Partly locked groups are **excluded**.
- The list text gets one line per item (CRLF + combined path) for the first 20 items; for every further item `"... "` is appended.
- n = the total number of *files* in the collected items.
- A dialog appears: Warning icon, buttons Yes and No. Text:
  `Mes_SureToDelAllFiles` + `"?"` + CRLF + CRLF + `Mes_SureToDelAllFiles2` + `": "` + n + CRLF + the list + CRLF + `Mes_NoWayToBack` + `" ! ! !"`.
- On Yes, each item is deleted (FL-053), with UI messages processed between items.

**FL-053 Delete an item from disk** (`FM: Del L1550–L1558`, `FM: DelFile L1543–L1549`). For each file of the item:
- delete its GPS info file (the sidecar, or the video itself when the GPS was embedded);
- delete the video file;
- delete the info path with its last 4 characters replaced by `.srt`.

Each deletion first clears all file attributes (including read-only), then deletes; errors are ignored. The item is then removed from the list (FL-050).

**FL-054 Set Lock / Unlock** (`Main: N_FileList_SetLockClick L3506–L3512`; `FM: SetLock L1582–L1603`).
- For every selected row (last to first) and every file in it: set (Set Lock) or clear (Unlock) the file's **read-only attribute** on disk, and set its lock flag to 1 or 0.
- The item is recomputed (lock state becomes 0, 1 or 2 per §4.7), then the window is re-laid out and repainted.

**FL-055 Group with…** (`Main: N_FileList_Group_WithClick L4116–L4119`; `UnitFormGroup: Popup L56–L89, ButtonGroupClick L95–L103, ListViewSelectItem L126–L138, TimerDrawRouteTimer L155–L182, FormKeyDown L188–L203`).
- Opens the modal Grouping dialog. It shows a copy of every main-list row (same columns, width + 8, same icons) with checkboxes:
  - rows selected in the main list are pre-ticked;
  - the current row is ticked, selected and focused.
- Ticking or selecting restarts a route-preview timer: 100 ms if fewer than 2 rows are ticked, else 1000 ms. The timer draws the routes of the ticked rows on the main map; selected rows are highlighted.
- Space toggles all selected rows' ticks to the opposite of the first selected row's tick.
- **Group** merges all ticked rows into one item (§4.6.4, "manual group") and closes the dialog. **Cancel** closes without changes.
- Afterwards the main list selection is cleared.

**FL-056 Ungroup** (`Main: N_FileList_UngroupClick L4108–L4113`; `FM: UngroupItem L1666–L1688`).
- Each selected multi-file item (last to first) is split: the first file stays, and every other file becomes its own new item. Everything is recomputed and re-sorted, and the selection is cleared.
- The split files keep no "do not regroup" mark. The next automatic grouping (FL-010, which runs only after new preview work) may merge them again.

**FL-057 Find Groups Automatically** runs §4.6.2 immediately in the UI thread, then notifies the list (`Main: N_FileList_Group_AutoClick L4122–L4124`).

**FL-058 Ungroup All** turns every file into its own item, then recomputes and sorts (`Main: N_FileList_Ungroup_AllClick L4127–L4129`; `FM: UngroupAll L1908–L1930`). The original sends no notification (OQ-2).

**FL-059 Show File in Explorer** (`Main: N_FileList_ShowExplorerClick L3953–L3957`; `UnitUtilsFile: ShowInExplorer L580–L585`).
- Uses the first file of the *current* row.
- If it exists, runs `explorer /e, /select, "<path>"`; otherwise opens its folder.

**FL-060 Property** (`Main: N_FileList_PropertyClick L3962–L3966`; `UnitFormFileProperty: Popup L143–L165, AddSubitem L85–L138, ButtonCopyClick L171–L181`). Opens **one non-modal Property window per selected row**.
- Caption: "Property " + Name.
- Grid rows (names are translation keys):
  - `Name`, `Duration` (the column text), `DateTime` (group start + time-zone shift).
  - **Single file:** the file block.
  - **Group:** `Grouped` = number of files, the combined block, then for each file a blank row and its block with names prefixed `"[i] "`.
- File block:
  - `File` = path; media-info lines without names.
  - `Size` = "<bytes> byte   (<MB, 1 decimal> Mb)".
  - `Locked` = `Locked_true` ("true") or `Locked_partial` ("partial"); omitted when 0.
  - `Started` / `Finished` (with time-zone shift) and `Duration`, prefixed "~" when not exact.
  - If there is GPS info: `MetaInfo` section with `File`, `DataFormat`, GPS points and G-Sensor points. If the GPS state is 2, also `Property_Track_Length`:
    - metric: rounded metres + " m";
    - British: miles to 3 decimals;
    - and `AvgSpeed` to 1 decimal + km/h or mph.
    Then its times.
  - If there is a GPS log: a "GPS Log" section with `File` and its times.
- Name-column width = longest name + 4 px; the value column fills the rest.
- **Copy All to Clipboard** copies all rows as TAB-separated cells with CRLF line ends.

**FL-061 Copy File to Computer** (Shift+C) (`Main: N_FileList_CopyClick L4502–L4512`).
- All files of all selected rows are collected into one combined pseudo-item. If there is at least one file, the Copy window opens for it.
- The Copy window asks for the destination with TFormSelectFolder, preset from registry `copyFolder`. Its internals are **out of scope** (Copy spec).

**FL-062 Save whole group** (`Main: N_FileList_SaveClick L4949–L4973`).
- If an item is currently loaded: the A–B selection becomes the whole timeline (0..1), the normal "Save selected fragment" command runs, and the B marker is reset.
- Otherwise: the Cutting window opens (restored if minimised), and every file of every selected row is added with its time window. If the window's list was empty, its selection is cleared.
- Cutting and saving are out of scope.

**FL-063 Repair** (`Main: N_FileList_RepairClick L4983–L4992`).
- Only when an item is currently loaded: opens the Repair window for the first file whose `durationState = 2`, or for the first file if there is none.
- Repair internals are out of scope.

**FL-064 Show Related GPS Log** (hidden in this version) (`Main: N_FileList_Track_GPSLogClick L3978–L3996`).
- Opens the GPS-log converter with all pool logs, unchecks them all and clears its selection.
- Then, for each selected row and each file with a GPS log, the matching log row is found by exact caption, checked, selected and scrolled into view.
- Converter internals are out of scope.

**FL-070 GPS of the selection** (`Main: GetSelectedItemGPSInfo L3675–L3695`).
- If the item now playing is the only selected row, its already-built movement records are used, including logs.
- Otherwise each file of each selected row is loaded from its GPS **info** path only, limited to that file's time window, and appended.
- The result is "has GPS" when at least one point exists.

**FL-071 Export Track** (`Main: N_FileList_Track_ExportClick L3703–L3723`).
- If there is no GPS (FL-070), the message `Mes_NoGPSInfo` ("No GPS Info to perform this action") is shown.
- Otherwise the save dialog opens:
  - Filter: concatenation of the export formats in this order: KML, PLT, GPX, DatDOD, three CSV variants, SRT.
  - Default file name: the track name with ":" replaced by "-", cleaned of characters invalid in file names.
- On OK: if the chosen name does not end (case-insensitive) with "." + the chosen format's default extension, that extension is appended. The file is written and its timestamp set (`WriteFileTime`).

**FL-072 Open Track in Google Earth** (`Main: N_FileList_Track_GoogleClick L3729–L3744`).
- If there is no GPS, the same message as FL-071 is shown.
- Otherwise a KML is written to `%TEMP%\Track <date-time for file names>.kml`; if the temp path is unknown, `C:\Windows\Temp\` is used. The file name is cleaned, then opened with the shell's default application.

**FL-073 Attach Track** (`Main: N_FileList_Track_SetClick L5231–L5239`; `FM: SetTrack L2207–L2248`).
- Playback stops and the track open dialog appears.
- On OK the chosen file is loaded as a track for the **current row**.
- **Time alignment:** let D_item = the item's real duration and the track span = [min, max]. The video files' finish times are all shifted by (track max − item finish), i.e. aligned to the track's end, when any of these holds:
  - |D_item − D_track| < 11 s;
  - the durations are within about 2 % of each other (|D_item / (D_track + D_item) − 0.5| < 0.01) and the finishes differ by more than (D_track + D_item) / 20;
  - the item finishes before track min + 11 s;
  - the track ends before item start + 11 s.
- Then every file's GPS info is replaced by this track: path = track file, statistics from the track cropped to the file's window, time = the track's time, marked previewed.
- The item is recomputed and re-loaded (FL-031).

**FL-074 Move video position** (`Main: N_FileList_Track_MoveClick L5246–L5264`; `FM: ShiftVideo L2274–L2288`).
- An input box appears, captioned with the menu caption. Its default is the zero duration formatted by `TimeDurationToStr(0, ".")`.
- A leading "-" means negative. The rest is parsed as a time of day in the system format; an invalid value counts as 0.
- If the result is non-zero: playback stops, every file's finish time of the current row is shifted by that amount (moving the video against GPS), and the item is recomputed and re-loaded.

**FL-075 Overlap time** (Ctrl+J, View → Timeline) (`Main: N_View_Movie_OverlapClick L5682–L5691`).
- Playback stops, then a 200 ms pause with messages processed.
- An input box appears: title `Mes_MovieOverlapTitle` ("Video overlap time"), prompt `Mes_MovieOverlapText` + ":" ("Enter time length in milliseconds:"), default = registry `trimTailMS` (default "0").
- On OK the text is stored **unvalidated** in `trimTailMS` and the current row is re-loaded.
- Effect: in internal-splitter group playback, each file's tail is cut by that many ms (§4.10). There is also an unexposed registry value `trimHeadMS` (default 0) that cuts each file's head.

**FL-080 View → File List** (F2) toggles the list area (§2.1). Its checked state is persisted under the item's name (bool; deleted when equal to the default "checked").

**FL-090 Unit strings.**
- The track-length suffix is `Kmeters` ("km") in metric mode, or the **first 2 characters** of `Miles` ("mi") in British mode. It is appended **without a space**.
- Changing the language or the units recomputes all rows (`FM: RefreshStrings L2358–L2369`; `Main: DoTranslate L4687–L4688`).

### G. Persisted settings

Registry key: per-user `\Software\DATAKAM PLAYER\` (`UnitReg: initialization`). Values are stored as strings; booleans as "true"/"false".

| Value | Type / default | Meaning | Source |
|---|---|---|---|
| `col<ID>_w` | int; absent = default | Column width | FL-023, FL-024 |
| `col<pos>_i` | int; absent = identity | Column ID at a display position | FL-023, FL-024 |
| `col_sort` | int, default 2 | Sort column ID | FL-025 |
| `col_sort_dur` | bool, default true | Ascending | FL-025 |
| `trimTailMS` | text (ms), default "0" | Overlap trim of each file's tail | FL-075 |
| `trimHeadMS` | int, default 0 (no UI) | Trim of each file's head | §4.10 |
| `N_View_FileList` | bool, default true | List visible | FL-080 |
| `copyFolder` | text | Copy destination (initialised at startup from a special folder) | FL-061 |
| `panel_Browser` | float, default 0.6 | Map/list height ratio, saved when the splitter moves | `Main: SplitterListViewMoved L2515–L2521`, `SplitterRightMoved L2524–L2533` (layout spec) |

---

## 4. Data rules

### 4.1 Data model (from the type info)

- **Item** (`TFileItem`):
  - display strings: dateTime, size, trackLength, avgSpeed, duration, pictureSize;
  - an ordered array of **files** (`TFileItemSubitem`);
  - a **group** summary record of the same type as a file.
- **File** = three blocks:
  - **video** (`TVideoFileItem`):
    - path; face (= file name without extension);
    - lock flag (0/1; for the group summary 0/1/2);
    - durationMS and durationState:
      - 0 = unknown;
      - 1 = estimated from file times (shown with "~");
      - 2 = the internal parser failed (warning icon);
      - 3 = exact;
    - previewed; picture size; fps; kbps; media-info strings; H.264 and AAC flags; GOP string and size;
    - **noGroup**;
    - a **time block**.
  - **info** (`TGPSLogFileItem`): GPS found inside the video or in a same-name sidecar. Path, lower-cased path, trackLength (m), avgSpeed (km/h), previewed, **gps state** (0 none, 1 points without a valid fix, 2 valid), format ID, GPS and G-sensor point counts, time block.
  - **gpsLog**: same type; a log from the pool assigned by time overlap.
- **Time block** (`TBlockDateTime`):
  - `fs` = file-system creation time, last-write time and size;
  - `real` = **duration** (days), **finish** (TDateTime), and the progress fractions **pStart** / **pFinish** of this file within its group.
  - A start time is always derived as finish − duration.
  - TDateTime values ≤ 2.0 count as "no date".

### 4.2 Adding a video file (`FM: AddVideoFile L401–L469`)

1. Create a single-file item with all fields empty.
2. face := file name without extension (`UnitUtilsStr: ExtractFileFace L217–L223`).
   - Quirk: if the face is exactly 8 characters, starts with "FI", and the number from character 3 onward is > 0, then the face becomes face + "." + characters 3–4, and characters 3–4 are overwritten with "LE". For example `FI012345` becomes `FILE2345.01`.
3. If the face starts with "fragment of " (case-insensitive):
   - remove that prefix;
   - cut at the last "(" if there is one;
   - append `(fr)`;
   - set **noGroup**. These are files saved by this program.
4. lock := 1 if the file is read-only, else 0.
5. Take the creation time, last-write time and size: from the directory entry during a scan, or by opening the file otherwise.
6. If the date of (write time + 12 h) is **1 January**, set noGroup (camera clock never set).
7. Set real.finish := write time and real.duration := write − creation.
   - If that is negative or more than 4 h, it is set to 0.
   - If it is > 0, durationState := 1 (estimate).
8. Compute the group summary (§4.7) and append the item.

### 4.3 Folder scanning

See FL-003 and FL-004.

### 4.4 Preview pipeline

**4.4.1 Video** (`FM: PreviewItemVideo L687–L747`). The file is opened with the internal container parser.
- **If its duration is > 0:**
  - durationState := 3 and real.duration := the parsed duration;
  - media info, picture size, fps, kbps, H.264 flag and GOP size are read;
  - AAC flag := the second track is audio with codec "MP4A".
  - If GPS is allowed and GPS metadata is found inside the file:
    - info.path := the video path;
    - statistics and time are taken from it;
    - for a parking-mode track that is longer than the video, the duration is extended to the track span.
  - Else, if GPS is allowed and the file has a GPS track: info.path := the video path, to be read in step 4.4.3.
- **Afterwards, in all cases** (also after a failure):
  - if durationState ≠ 3, it becomes **2**;
  - previewed := true;
  - if write − creation < duration, then creation := write − duration.

**4.4.2 GPS log from the pool** (`FM: PreviewGPSLog L795–L858`). The log is loaded and its statistics and time span extracted (`FM: ExtractGPSTime L2304–L2319`).
- If it has fewer than 2 GPS points and fewer than 2 G-sensor points, its times are cleared. It is marked previewed.
- If its duration is > 0, it is tested against every file of every item:
  - **v** = the weighted overlap between the video window and the log window (§4.5, `GetIntersection` with name weighting).
  - **Assign as the file's gpsLog** when all of these hold: v > 2 s, v is larger than the overlap with the file's current gpsLog, and v > 0.1 × the video duration.
    - The clock is then synchronised (§4.11).
    - If the file has no GPS info yet, its info statistics are filled from the log cropped to the file's window, and the info time is set from the log.
  - **Assign as the file's GPS info** when all of these hold:
    - the log span − 11 s < min(raw overlap, video duration), i.e. the log lies within the video;
    - the lower-cased faces of the video and the log are equal;
    - the file has no info yet, or the log's extension rank (§4.5) is higher than the current info's.
    - In that case the gpsLog is cleared if it was the same file, and the clock is synchronised.
  - Every item that changed is recomputed.

**4.4.3 GPS info of a video file** (`FM: PreviewItemInfo L885–L939`). If GPS is allowed:
- Get the candidate list from the sidecar finder (§4.5). If the info path was already set (embedded GPS), it is placed first.
- Only the **first** candidate is used: it becomes the info path and is loaded.
- If it fails, or has fewer than 2 GPS and fewer than 2 G-sensor points, the info path is cleared.
- Otherwise its file times are read.
  - If the log does not overlap the video in time, the log span is more than 1.1 × the video duration, and some file already uses this log as its gpsLog, then the info is cleared (it is a shared trip log, handled by 4.4.2).
  - Else its statistics and time are extracted.
- previewed := true.

### 4.5 Sidecar matching (`UnitGPSManager`)

- **Known GPS extensions** (lower-case; `TGpsInfoManager.Create L95–L114`, initialization L931–L992):
  `gpx, kml, map, gmap, dat, log, plt, txt, nmea, ifo, gps, srt`.
  `HasFileExt` = the lower-cased extension is in this list (L768–L779).

**Candidate search** (`FindFilesForVideo L131–L142`):
- Two passes: (1) the video's own folder only; (2) GPS sub-folders.
- In each pass, for each extension in the order above, one candidate is chosen (FindFileExt). Every candidate that exists is appended, so the list may hold several.

**Direct lookup** (`FindFileExtDirect L251–L288`), with face = the video path without extension:
- Folders, in order:
  - pass 1: the video folder;
  - pass 2: `<dir>\GPS`, then `<parent>\INFO`, then `<grandparent>\gps`.
- Names tried in each folder, in order:
  1. the face;
  2. face + "." (giving `name..ext`);
  3. "gps" + the face without its first 4 characters (`FILE0123` → `gps0123`);
  4. the face without its last character.
- The first existing `<folder>\<name>.<ext>` wins. Existence checks use a cached, lower-cased directory listing, so matching is case-insensitive (`UnitFileListCache` L37–L122).
- The cache is cleared after the initial scan and after each automatic grouping.

**Choice between this segment's log and the previous one** (`FindFileExt L186–L223`). Times come from the cached creation/write times: if write ≤ creation, or the span is ≥ 4 h, the start becomes write − 1 min (`GetFileTime L178–L185`).
1. gps := the direct lookup for the video.
2. gps_prev:
   - if gps exists: the same name with its last number decreased by 1, if that file exists;
   - else: the direct lookup for the video whose last number is decreased by 1 (`GetNearFile L155–L176`, keeping zero padding).
3. If gps exists, the next-numbered video exists, the overlap of this video with gps is < 2 s and the overlap of the next video with gps is > 2 s, then gps is dropped.
   - Quirk: the number search includes the extension, so for `.MP4` it bumps "4" and this check never fires (OQ-5).
4. Return gps_prev if its overlap with the video is > 0 and greater than gps's overlap; otherwise return gps (possibly empty).

**Overlap measure** (`FM: GetIntersection2 L764–L787`):
- 0 unless both windows have valid dates (start > 2.0), start < finish, and both durations are > 0.
- ov = min(finishes) − max(starts). If ov > 2.0 days, it is recomputed from (finish − duration) starts.
- If ov > 11 s and ov > 0.9 × the shorter duration, ov := the longer duration.
- ov := min(ov, the longer duration).
- With name weighting:
  - ×2 if the log extension is `.log`, `.gpx` or `.plt`;
  - then ÷ (1 + filename distance / 100).

**Extension rank** (higher = preferred) (`FM: GetFileExtPower L1003–L1013`): `.kml` 1, `.srt` 2, `.txt` 3, `.dat` 4, other 5, `.log` 6, `.plt` 7, `.gpx` 8.

**Filename distance** (`FM: GetFileDistance L961–L979`):
- Compare the two paths up to their last "." (each).
- Distance = (max length − length of the common prefix) + 10 for every `\` in either remainder.
- Video distance (`FM: GetVideoFileDistance L982–L986`) adds 1000 when the picture sizes differ.

### 4.6 Grouping

**4.6.1 Fast grouping** (`FM: FindGroupsFast L1712–L1745`).
- Run 4 passes: link distance 3 s with sort by full path, 3 s with sort by name, 10 s with sort by full path, 10 s with sort by name. The sorts are temporary, ascending.
- In each pass, walk adjacent items i and i+1. Let A = the last file of item i and B = the first file of item i+1. Merge B's item into A's item when both hold:
  - **connected by time** (`FM: IsConnectedByTime L1700–L1704`, `CheckIt L1692–L1699`), first with file-system times (creation as start, write as finish), else with real times (finish − duration, finish). Every one of these must hold:
    - gap = start(B) − finish(A) lies within [−dist/2, +dist];
    - each file lasts more than 1 s;
    - start(B) > start(A) + 1 s;
    - finish(B) > finish(A) + 1 s;
  - the video filename distance of (A, B) is ≤ 99. A different resolution always adds 1000, so it fails.
- Merging appends all of item i+1's files to item i, removes item i+1 keeping order, and checks the same i again. A changed item is recomputed when moving on.
- noGroup is **not** checked here.
- Fast grouping runs after each folder scan, after the last video preview, after the last info preview, and at the start of automatic grouping.

**4.6.2 Automatic grouping** (`FM: FindGroupsAuto L1811–L1872`, `GetL L1763–L1810`).
1. Fast grouping; clock fixing (§4.11); split erroneous groups (4.6.3).
2. Repeat: over all pairs of items (i, z) with i < z, compute **L(i, z)** (below). Keep the pair with the smallest L that qualifies:
   - L < 6 min qualifies;
   - 6 min ≤ L < 6 h qualifies only if L × 4 is less than the duration of item i or of item z;
   - otherwise the pair does not qualify.
   If a pair is found, merge z into i (4.6.4, using time-union) and repeat; otherwise stop.
3. If anything merged: recompute all items, then run clock fixing and erroneous-group splitting again.
4. Recompute all items, sort, notify.

**L(i, z)** is in days; 1.0 means "no link". Use the real times of the item summaries:
- s = finish − duration (taken as 0 if the duration is 0); f = finish.
- L = 1 if i = z, either item has **noGroup**, f ≤ s for either, or either s ≤ 2.0.
- L = 1 if one span contains the other within 0.5 s, in either direction.
- d = the video filename distance (last file of i, first file of z); if d = 1, treat it as 0. Weight w = 1 + d/10.
- If z starts inside i and ends after it (0 < s_z < f_i < f_z): L = w × (f_i − s_z). If L > 60 s, then L = 1.
- Else if z starts before i and ends inside it (0 < s_i ≤ f_z ≤ f_i): L = w × (f_z − s_i). If L > 60 s, then L = 1.
- Else (disjoint): L = w × min(1, |f_i − s_z|, |f_z − s_i|).

**4.6.3 Split erroneous groups** (`FM: UngroupErrors L1878–L1903`).
- For each item with 2 or more files, check consecutive files z and z+1: if |finish(z) − start(z+1)| > 12 h, files z+1… are moved to a new item appended at the end.
- Both items are recomputed and the scan continues with the next item, including the new ones.

**4.6.4 Manual group / merge** (`FM: GroupItems L1633–L1661`).
- Given a list of item indices, the first is the target.
- Every other valid index (≠ target) has all its files appended to the target. For automatic merges, the target's time block is first unioned with the other's (`IncBlockTime L1617–L1628`): finish = max, start = min.
- The source items are removed (highest first).
- Manual grouping (from the dialog) then recomputes the target and sorts.
- No compatibility or noGroup checks are made for manual grouping.

**4.6.5 Ungroup** (`FM: UngroupItem L1666–L1688`) and **Ungroup All** (`FM: UngroupAll L1908–L1930`): see FL-056 and FL-058.

### 4.7 Group summary computation (`FM: CalculateGroup L1970–L2099`)

1. **Order files:** bubble-sort by start time (finish − duration). On equal starts, order by full path ascending.
2. For every file, durationMS := round(real duration × 86 400 000).
3. **Single file:** the summary is a copy of the file.
4. **Several files:** start from an empty summary with durationState = 3. For each file, in order:
   - **Display name composition.** The index of a name is its trailing run of characters in `0–9`, `-` or `.`, if 1–5 characters long, read as a number; otherwise there is no index (`GetInd L1944–L1953`). Two faces are "neighbours" when, after their common prefix, both remainders consist only of digits, `-` or `.` (`IsFilesNeigbor L1957–L1969`).
     - cur := index(face) if this is file 0 or a neighbour of the previous face; otherwise "none".
     - If prev and cur both exist and |prev − cur| < 1.99 (a consecutive number): remember cur as pending, and if this is the last file, append `-<floor(cur)>`.
     - Otherwise:
       - first flush any pending number as `-<n>`;
       - then file 0 → append its face;
       - a later file with an index → append `, <floor(cur)>`;
       - a later file without an index → append ` (<file count>)`, but only if it is the last file (other unnumbered files add nothing).
     - Examples: `FILE0001`, `FILE0002`, `FILE0003` → `FILE0001-3`; `A_0001`, `A_0005` → `A_0001, 5`.
     - Names longer than 50 characters become the first 50 + `...`.
   - Path := all paths joined with ", ".
   - Size and durationMS: summed.
   - durationState := the minimum over files.
   - Time block: union (finish = max of finishes, start = min of starts).
   - Lock count: summed.
   - Picture width, height, fps and kbps: maximum.
   - GPS: for a file with info or gpsLog:
     - append its info path (", "-joined);
     - if its gps state is 2, add its trackLength, and when its avgSpeed > 0 add trackLength / avgSpeed to the total GPS time;
     - gps state := max; GPS time block union; point counts summed.
   - **Lock state:** 0 if no file is locked; 2 if the count is less than the file count; else 1.
   - **Average speed** := total track / total GPS time, or "none" (−1) if the GPS time is 0.
5. **Progress fractions:** let G = the summary's real duration (forced to 1.0 if ≤ 0) and F = the summary finish. For every file's video, info and gpsLog blocks:
   - pFinish = 1 − clamp01((F − finish) / G);
   - pStart = pFinish − clamp01(duration / G).
6. Format the display strings (§4.8).

### 4.8 Column strings (`FM: BinaryToStr L2107–L2182`)

- **Date and time:**
  - t = summary finish + the user time-zone shift, minus the duration if the duration is > 0.
  - If t > 2.0: m = round(frac(t) × 1440) minutes, and the time part = `" " + 2-digit(m div 60) + ":" + 2-digit(m mod 60)`.
  - Prefix: `today` if the date part of t equals today's date part, `yesterday` if it is one less, otherwise the system short date (DateToStr).
  - A value of 24:00 is possible when rounding reaches 1440.
  - If t ≤ 2.0 the string stays empty.
- **Duration:**
  - If durationMS > 0 and durationState ≠ 0: `TimeToStrSmall(ms, seconds = true)` (UnitUtilsStr L329–L343) gives:
    - with days: `d` + `hh` + `:` + `mm`;
    - else with hours: `h` + `:` + `mm`;
    - else `m`;
    - then `:` + `ss`;
    - the separator is the system time separator.
  - Prefixed with `~` when durationState = 1.
  - Otherwise `?`.
- **Size:**
  - round(bytes / 1024 / 1024) as an integer.
  - If longer than 3 digits, a back-quote is inserted before the last 3 digits (`1234` → ``1`234``).
  - Empty if the size is negative.
- **Resolution:** `WxH`, or empty when both are 0.
- **Trip:**
  - km = trackLength / 1000; in British mode divide by 1.609344.
  - > 100 → integer; > 2 → 1 decimal; > 0 → 2 decimals (decimal separator always `.`); else empty.
  - When non-empty, the unit is appended (FL-090).
- **Speed:**
  - avgSpeed; ÷ 1.609344 in British mode.
  - ≥ 100 → integer; > 0 → 1 decimal; else empty. No unit.

### 4.9 Sorting

See FL-025. The keys are computed from the group summary.

### 4.10 Time mapping across the files of a group

- **Progress p ∈ [0, 1]** is wall-clock based over the group span [min start, max finish]. Gaps between files are included.

Conversions in Main:
- **ProgressToSubitemIndex** (L2220–L2230): the first file i with p < pFinish(i) (or the last file). It is chosen if p ≥ pStart(i), or i = 0, or p ≥ pFinish(i−1). A p that falls in a gap maps to the **next** file. The fallback is 0.
- **ProgressToSystemTime** (L2241–L2246): finish − (1 − p) × G.
- **ProgressToVideoTime** (L2233–L2238): round(p × G × 86 400 000) ms, i.e. elapsed wall-clock time since the group start.
- **ProgressToFile** (L2251–L2258): the file name, prefixed `"[i+1] "` when the group has more than one file.
- **Global/local conversion:** global = pStart + local × (pFinish − pStart) (`GetGlobalProgressFromVideo L2168–L2171`, `GetLocalProgressVideo L2180–L2189`).

Playback modes:
- **Internal-splitter group playback** (`UnitSources: TSourceGroup.Add L382–L672`, used by `Main: RenderFile L2322–L2341`):
  - Files are concatenated back-to-back in stream time, in list order. Wall-clock gaps are *not* reproduced.
  - The first file defines the tracks. A file whose tracks are incompatible (type, FourCC, format block or decoder info differ) makes the whole group fall back to playing only the requested file.
  - Each file is cut to [trimHeadMS, duration − trimTailMS].
  - Stream time maps to (file, local progress) via `TimeToProgress L697–L719` and back via `ProgressToTime L722–L729`.
- **Per-file DirectShow mode:**
  - Seeking to another file re-renders it.
  - At a file's end, playback jumps to max(next pStart, current pFinish), so an overlapping head is skipped.
  - When the graph reports a duration for a file with durationState = 2, the file becomes state 3 with that duration (`FM: SetFileDuration L2324–L2340`).

### 4.11 Clock fixing (affects the Date column and grouping)

- **SyncVideoByGPS** (`FM L1018–L1055`), when a GPS info or log is attached:
  - If the video has no duration (and is not exact) but the GPS info has one, the video takes it.
  - The video finish := write time + the GPS clock shift (`GetClockShift L944–L956`: GPS finish − file write time, or 0 when the GPS window lies inside the file window or the dates are invalid).
  - If the durations match within 11 s, or are within 1 %, the video finish := the GPS finish and the GPS duration := the video duration.
  - Else, if the windows do not overlap, the video finish := the GPS finish.
- **FixClockChanged** (`L1080–L1129`): within a group, the first exact file (state 3) whose write − creation exceeds 4 h but whose duration is under 4 h defines a correction d = (write − creation) − duration, which is added to its creation time. The other files' times are shifted by the same d when they stay within limits.
- **ShiftsWithoutGPS** (`L1172–L1196`, `DoShiftLikeNear L1141–L1171`): in groups of 2 or more files, a file without a valid GPS fix, or without any GPS, gets its finish shifted by (median clock shift of its GPS-equipped siblings − its own shift).

---

## 5. Out of scope (referenced only)

- Copy window (`UnitFormCopy`) and `TFormSelectFolder`.
- Cutting/save (`UnitFormCropping`, `N_File_Save_VideoClick`).
- Repair (`UnitFormRepair`).
- GPS-log converter (`UnitFormGPSLog`, `N_File_Convert_GPSClick`).
- Map drawing (`SetRoute`, `RedrawRoute`).
- Playback (`ButtonPlayClick`, `ButtonStopClick`, `Seek`, `RenderFile`).
- Firmware check `CheckFlashForRegistrator`.
- Auto-update.
- The map/list splitter ratio (layout spec).

All handlers wired to list-area components are covered above:
- **Main:** SplitterListViewMoved, N_FileList_Refresh(2), N_FileList_Clear(2), N_View_FileList, N_View_Movie_Overlap, PopupMenuListViewPopup, every `N_FileList_*` handler, ListViewClick, ListViewColumnClick, ListViewColumnDragged, ListViewCustomDraw, ListViewData, ListViewDblClick, ListViewKeyUp, ListViewSelectItem.
- **TFormGroup:** all.
- **TFormFileProperty:** all.
- **TFormSelectFolder:** delegated to the Copy spec.

---

## 6. Open questions

1. **OQ-1 Sort of equal start times inside a group.** `CalculateGroup L1979–L1983` swaps on an unequal-name test as decompiled; the real comparison is presumably "name greater". Assumed: ascending full path. Check the disassembly around 0x535e3c if exactness matters.
2. **OQ-2 Owner-data refresh without notification.** After Remove from List, Clear, Ungroup, Ungroup All or Set Lock, the original never resets the row count directly. It waits for the next manager notification; FindGroupsAuto and the preview tasks notify, the others do not. Grep of all callers: only `N_FileList_RefreshClick` and `TimerOnStartTimer` install the callback, and nothing else calls `OnFileManagerChanges`. I could not determine from VCL behaviour what rows beyond the manager's count show meanwhile. **Recommendation:** refresh immediately. Referenced from FL-009, FL-050 and FL-058.
3. **OQ-3 Selection after a header-click sort.** `ListViewColumnClick` does not re-select by name, so the selection stays on the row index. Re-selection by name only happens on the next manager notification (FL-012). Confirm against the real app whether the highlight jumps.
4. **OQ-4 Ctrl+A** only fires through the application-level shortcut hook when the main or full-screen form is active; not verified for when the list lacks focus.
5. **OQ-5 GetNearFile quirk.** It bumps the last digit run of the *full path including the extension*, so the "next video" overlap rule in sidecar choice (§4.5 step 3) never fires for `.MP4` files. Faithful reproduction or fix? Product decision.
6. **OQ-6 Formats without a file extension** (RegV, DOD, Panorama, CamBox, …) are presumably detected by content or inside MOV files, not as sidecars. Not verified (UnitGPSManager format constructors).
7. **OQ-7 TimeDurationToStr(0, ".")**, the default text of the Move video position box: exact format not decoded (UnitUtilsStr L303–L319).
8. **OQ-8 TBlockDateTime sub-field names** are unnamed in the debug info (`?0` structs). The meanings (creation, write, size, duration, finish, pStart, pFinish) were inferred from the arithmetic in IncBlockTime, AddVideoFile, CalculateGroup and BinaryToStr, and are consistent across them.
9. **OQ-9 Property window** "(position)" prefix in `AddStartFinish L73–L84`: the condition is unclear. Some keys ("points", "Ok", "GPS Log") are missing from TRANSLATE_EN, so lookup fallback behaviour applies.
10. **OQ-10 File-list cache sort order** (`UnitFileListCache: LoadDir L102–L122`) is assumed ascending and case-insensitive; the comparison did not decompile cleanly. This matters only for performance.
