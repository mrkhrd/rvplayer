/* C interface to the RV Player Rust core (crates/rv-ffi). All strings are UTF-8.
 * Strings returned by rv_* functions must be released with rv_string_free.
 * Frame buffers returned in RvEvent must be released with rv_frame_free. */
#ifndef RV_FFI_H
#define RV_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* File summary and GPS/G-sensor track as JSON (see rv-ffi/src/lib.rs for the schema).
 * Never NULL: read errors are reported in the "error" field. */
char *rv_file_info_json(const char *path);

/* Sorted list of video files (.mov/.mp4) in a folder, as a JSON array of paths; "[]" on error. */
char *rv_list_videos_json(const char *dir);

void rv_string_free(char *s);

typedef struct RvPlayer RvPlayer;

enum RvEventKind {
    RV_EVENT_NONE = 0,
    RV_EVENT_OPENED = 1,
    RV_EVENT_FRAME = 2,
    RV_EVENT_END_OF_STREAM = 3,
    RV_EVENT_ERROR = 4,
};

typedef struct RvEvent {
    int32_t kind;
    uint64_t generation;
    /* OPENED: stream size; FRAME: frame size. */
    uint32_t width;
    uint32_t height;
    /* FRAME: presentation time in seconds. */
    double time;
    int32_t key;
    /* FRAME: packed RGBA, width * height * 4 bytes; release with rv_frame_free(rgba, rgba_len). */
    uint8_t *rgba;
    size_t rgba_len;
    /* OPENED */
    double duration;
    double frame_rate;
    /* ERROR: message; release with rv_string_free. */
    char *error;
} RvEvent;

/* Opens a video on a background decoding thread; the first frame is decoded from `start` seconds. */
RvPlayer *rv_player_open(const char *path, double start);
/* Restarts decoding at `time`; queued frames from before carry an older generation. */
void rv_player_seek(RvPlayer *player, double time);
uint64_t rv_player_generation(const RvPlayer *player);
/* Fills `out` with the next event without blocking; returns 0 when there is none. */
int32_t rv_player_poll(RvPlayer *player, RvEvent *out);
void rv_player_close(RvPlayer *player);

void rv_frame_free(uint8_t *rgba, size_t len);

/* ---- File manager (spec docs/spec/file-list.md) ------------------------------------------------
 * The list of items (single files or groups) behind the main window's file list. A background
 * thread scans folders, previews files and groups them; the UI polls rv_fm_version and re-reads
 * the rows when it changes. Times are Delphi TDateTime values: days since 1899-12-30, local time;
 * values <= 2.0 mean "no date". Item indices are row numbers in the current sort order. */
typedef struct RvFileManager RvFileManager;

/* FL-001/FL-002: auto_search queues the FL-003 locations; gps = 0 for the video-only profile. */
RvFileManager *rv_fm_create(int32_t auto_search, int32_t gps);
/* Stops the background thread and frees the manager. */
void rv_fm_destroy(RvFileManager *fm);
/* FL-005: a folder is queued for scanning, a file is added (or re-previewed if present). */
void rv_fm_add(RvFileManager *fm, const char *path);
/* FL-009 */
void rv_fm_clear(RvFileManager *fm);
/* FL-025: column ID 0..7, ascending != 0. The manager keeps itself sorted by this key. */
void rv_fm_set_sort(RvFileManager *fm, int32_t column, int32_t ascending);
/* Incremented on every change the UI should show (FL-012 notification). */
uint64_t rv_fm_version(const RvFileManager *fm);
/* 1 when the background thread has nothing left to do (FM: WaitForPreview). */
int32_t rv_fm_is_idle(const RvFileManager *fm);
/* Current rows and status as JSON, see rv-ffi/src/fm.rs for the schema. */
char *rv_fm_rows_json(RvFileManager *fm);
/* One item with its files (time block, progress fractions, info paths) as JSON; "null" if the
 * index is out of range. */
char *rv_fm_item_json(RvFileManager *fm, int32_t index);
/* Commands on rows; `indices` are row numbers, `count` how many. */
void rv_fm_remove(RvFileManager *fm, const int32_t *indices, int32_t count);                 /* FL-050 */
void rv_fm_set_lock(RvFileManager *fm, const int32_t *indices, int32_t count, int32_t lock); /* read-only flag */
void rv_fm_group(RvFileManager *fm, const int32_t *indices, int32_t count);                  /* §4.6.4 manual */
void rv_fm_ungroup(RvFileManager *fm, const int32_t *indices, int32_t count);                /* FL-056 */
void rv_fm_ungroup_all(RvFileManager *fm);                                                    /* FL-058 */
void rv_fm_group_auto(RvFileManager *fm);                                                     /* §4.6.2 */

#ifdef __cplusplus
}
#endif

#endif /* RV_FFI_H */
