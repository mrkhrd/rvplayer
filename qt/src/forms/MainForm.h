// TFormMain: the main window, built from the original TFORMMAIN.dfm. Handler methods keep the
// original names so each one maps to its requirements in docs/spec. The implementation is split
// by area: MainForm.cpp (layout), MainForm_Timeline.cpp (seek bar, graphs, gauges, play speed),
// MainForm_FileList.cpp (file list and its toolbar).
#pragma once

#include "core/Group.h"
#include "core/MovingInfo.h"
#include "draw/Bitmaps.h"
#include "draw/Graphics.h"
#include "vcl/Form.h"

#include <vector>

class FileListView;
class FormAbout;
class FormCropping;
class FormRepair;
class FormGPSLog;
namespace gpsfiles {
struct Source;
}
class FormDetails;
class FormWhatsNew;
class QKeyEvent;
class QWheelEvent;
class FileManager;
class MapView;
class Playback;
class QToolButton;
class VideoView;

class MainForm : public vcl::Form {
    Q_OBJECT
    friend class MainFormTest;  // qt/tests drive the form through its internals

public:
    explicit MainForm(QWidget *parent = nullptr);

    // Spec: shell-settings-forms.md TR-020…TR-029 (texts that do not come from the form file).
    void doTranslate();

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void N_View_FileListClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Profile_GPSClick(vcl::Event *e);
    Q_INVOKABLE void SplitterRightMoved(vcl::Event *e);
    Q_INVOKABLE void SplitterListViewMoved(vcl::Event *e);

    // Timeline
    Q_INVOKABLE void N_View_Graphics_SClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Graphics_ShowClick(vcl::Event *e);
    Q_INVOKABLE void TrackBarPlaySpeedChange(vcl::Event *e);

    // File list
    Q_INVOKABLE void PopupMenuListViewPopup(vcl::Event *e);
    Q_INVOKABLE void TimerOnStartTimer(vcl::Event *e);
    Q_INVOKABLE void N_File_OpenClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_RefreshClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_ClearClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_SetLockClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_UngroupClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Ungroup_AllClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Group_AutoClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_ShowExplorerClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_PropertyClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelDiskClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelDiskAllClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Group_WithClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_CopyClick(vcl::Event *e);

    // Commands (MainForm_Commands.cpp)
    Q_INVOKABLE void N_Help_AboutClick(vcl::Event *e);
    Q_INVOKABLE void N_Help_WhatsNewClick(vcl::Event *e);
    Q_INVOKABLE void N_Help_Site_BrandClick(vcl::Event *e);
    Q_INVOKABLE void N_Help_Site_MyClick(vcl::Event *e);
    Q_INVOKABLE void N_Settings_ClearClick(vcl::Event *e);
    Q_INVOKABLE void N_Playback_Save_FrameClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Movie_OverlapClick(vcl::Event *e);
    Q_INVOKABLE void N_File_Save_VideoClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_SaveClick(vcl::Event *e);
    Q_INVOKABLE void N_File_Repair_VideoClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_RepairClick(vcl::Event *e);

    // GPS tracks (MainForm_Track.cpp)
    Q_INVOKABLE void N_File_Convert_GPSClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Track_GPSLogClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Track_SetClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Track_MoveClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Track_ExportClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_Track_GoogleClick(vcl::Event *e);

    // Coordinates (MainForm_Coords.cpp)
    Q_INVOKABLE void N_View_Coord_FlipXClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Coord_ShiftXClick(vcl::Event *e);
    Q_INVOKABLE void N_View_MarkerURLSetupClick(vcl::Event *e);
    Q_INVOKABLE void N_View_MarkerURLOpenClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Coord_CopySetupClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Coord_CopyClick(vcl::Event *e);
    Q_INVOKABLE void ListViewSelectItem(vcl::Event *e);
    Q_INVOKABLE void ListViewClick(vcl::Event *e);
    Q_INVOKABLE void ListViewDblClick(vcl::Event *e);
    Q_INVOKABLE void ListViewColumnClick(vcl::Event *e);
    Q_INVOKABLE void ListViewColumnDragged(vcl::Event *e);

    // Map
    Q_INVOKABLE void N_View_GoogleMapsClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Maps_OpenKMLClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Maps_PostActionClick(vcl::Event *e);

    // View, video zoom, full screen (MainForm_View.cpp)
    Q_INVOKABLE void DoTranslate(vcl::Event *e);
    Q_INVOKABLE void N_View_TimeZone_LocalClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Measu_MetricClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Arrow_SmallClick(vcl::Event *e);
    Q_INVOKABLE void N_View_TimePoints_1mClick(vcl::Event *e);
    Q_INVOKABLE void N_Playback_PlayNextClick(vcl::Event *e);
    Q_INVOKABLE void N_Playback_PauseOnClickClick(vcl::Event *e);
    Q_INVOKABLE void N_File_ExitClick(vcl::Event *e);
    Q_INVOKABLE void TrackBarVolumeChange(vcl::Event *e);
    Q_INVOKABLE void ButtonVideoZoomOutClick(vcl::Event *e);
    Q_INVOKABLE void N_VideoZoom_LetterboxClick(vcl::Event *e);
    Q_INVOKABLE void N_VideoZoom_OtherClick(vcl::Event *e);
    Q_INVOKABLE void VideoWindowWMRMouseDown(vcl::Event *e);
    Q_INVOKABLE void VideoWindowWMRMouseMove(vcl::Event *e);
    Q_INVOKABLE void VideoWindowWMRMouseUp(vcl::Event *e);
    Q_INVOKABLE void TimerMouseHideTimer(vcl::Event *e);
    Q_INVOKABLE void N_FullScreenClick(vcl::Event *e);

    // Playback
    Q_INVOKABLE void ButtonPlayClick(vcl::Event *e);
    Q_INVOKABLE void ButtonStopClick(vcl::Event *e);
    Q_INVOKABLE void N_Playback_PlayClick(vcl::Event *e);
    Q_INVOKABLE void N_Playback_StopClick(vcl::Event *e);
    Q_INVOKABLE void N_Playback_LeftClick(vcl::Event *e);
    Q_INVOKABLE void ButtonFrameNextClick(vcl::Event *e);
    Q_INVOKABLE void ButtonFramePrevClick(vcl::Event *e);
    Q_INVOKABLE void TimerTimer(vcl::Event *e);
    Q_INVOKABLE void ImageSeekMouseDown(vcl::Event *e);
    Q_INVOKABLE void ImageSeekMouseMove(vcl::Event *e);
    Q_INVOKABLE void ImageSeekMouseUp(vcl::Event *e);
    Q_INVOKABLE void OnImageMouseLeave(vcl::Event *e);
    Q_INVOKABLE void PopupMenuGraphicsPopup(vcl::Event *e);
    Q_INVOKABLE void ButtonZoomInClick(vcl::Event *e);
    Q_INVOKABLE void ScrollBarZoomChange(vcl::Event *e);
    Q_INVOKABLE void N_View_Time_SetAClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Time_LoopABClick(vcl::Event *e);
    Q_INVOKABLE void N_View_Refresh_SlowClick(vcl::Event *e);


protected:
    bool eventFilter(QObject *watched, QEvent *ev) override;
    QWidget *createControl(const vcl::DfmObject &def, QWidget *parent) override;
    void dragEnterEvent(QDragEnterEvent *e) override;
    void dropEvent(QDropEvent *e) override;

private:
    enum class Profile { GpsG, Gps, Video };
    Profile profile() const;
    bool mapVisible() const;
    void alignControlPanel();
    void persistSplitters();
    void setupGlyphs();
    QWidget *widget(const char *name) const { return find<QWidget>(name); }

    double browserH = 0.6;  // map height as a fraction of PanelRight2 (panel_Browser)

    // --- Timeline (MainForm_Timeline.cpp) ---
    void createSpeedButtons();
    void setSpeedButtonTag(QToolButton *b, int tag);
    void rebuildGraphs(bool user);
    void layoutGraphs();   // GM-031 stacking
    void drawTimeline();   // seek bar and graph backgrounds (FormResize)
    void drawCursors();    // seek pin and graph cursors (every timer tick)
    void clearGauges();    // ClearAll: needles off, boxes empty
    QVector<draw::Band> bands() const;

    draw::SeekBitmap seek;
    draw::SpeedBitmap speedBitmap;
    draw::AzimutBitmap azimutBitmap;
    draw::GSensorBitmap gsensorBitmap;
    struct GraphImage {
        draw::Graphic graphic;
        vcl::Image *image;
    };
    std::vector<GraphImage> graphs;
    QList<QToolButton *> speedButtons;
    double zoomL = 0, zoomR = 1;  // visible part of the group (GM-070)
    double position = -1;         // play position as a fraction of the group; < 0 when nothing is open

    // --- Playback (MainForm_Playback.cpp) ---
    void setupPlayback();
    void loadGroup(const Group &g);
    double currentPosition();
    void seekTo(double p, bool callTimer, bool force = false);
    void stopPlayback(bool clearSelection);
    void updateCaption();
    void updateGauges(const MovingValue &v);
    void setupZoomBar();
    void pauseForStep();
    // GM-120: the details popup for the timeline position under the mouse.
    void showDetails(QWidget *image, int x);
    void hideDetails();
    FormDetails *details = nullptr;
    double detailsCenter = -1, detailsWidth = 0;  // window of its mini-graph (progress)
    int detailsColumns = 0;

    Playback *playback = nullptr;
    VideoView *video = nullptr;
    MovingInfo moving;
    bool pinGet = false;  // the seek pin is being dragged
    double pinGetValue = 0;
    double lastPosition = 0;
    double selectL = 0, selectR = 0;  // A-B selection
    double zoomK = 2;
    bool playSpeedEnabled = false;

    // --- View (MainForm_View.cpp) ---
    void addLanguages();
    void addTimeZones();
    void buildPopupCopies();
    QSize nativeVideoSize() const;
    void layoutVideo();
    void showTimelineInFullScreen(bool show);
    bool routeWheel(QWheelEvent *e);
    bool extraKey(QKeyEvent *k);
    class AppEventFilter : public QObject {
    public:
        explicit AppEventFilter(MainForm *f) : QObject(f), form(f) {}
        bool eventFilter(QObject *watched, QEvent *ev) override;

    private:
        MainForm *form;
    };

    double tzShift = 0;  // GM-093, days added to displayed clock times
    double videoZoom = 1, videoDx = 0.5, videoDy = 0.5;  // PV-060
    bool videoGrab = false;
    QPoint grabPos;
    vcl::Panel *fullScreen = nullptr;

    // --- Map (MainForm_Map.cpp) ---
    MapView *mapView() const;
    void setupMapMenu();
    // GM-152: the map's notifications.
    void mapLoaded();
    void mapViewChanged(bool finished);
    void mapClicked(double lat, double lon);
    void setRoute();     // GM-160
    void redrawRoute();  // GM-161
    void updateMapMarker(const MovingValue &v, bool timerTick);  // GM-165, GM-167
    void drawMarkersAB();                                        // GM-168
    // FL-055: routes of several rows while choosing them in the Grouping dialog.
    void previewRoutes(const QList<int> &rows, const QSet<int> &highlighted);
    void previewTrackFiles(const QStringList &paths, const QList<bool> &selected);
    void drawPreview(const QStringList &keys, const QList<bool> &selected);
    QHash<QString, QVector<QVector<GpsSample>>> previewCache;
    bool mapWheel(int delta, QPoint global);                     // GM-170

    struct MapViewport {
        double top = 0, left = 0, bottom = 0, right = 0;  // degrees
        double x = 0, y = 0;                              // centre lon/lat
        int w = 0, h = 0, zoom = 0;
        double metresPerPixel = 0;
        bool valid = false;
    } viewport;
    bool mapFollow = true;    // the car is followed while it stays in view
    bool mapRecentre = true;  // gmArrowInCenter: centre on the next tick (after a seek)
    struct {
        bool valid = false;
        double lat = 0, lon = 0;
    } lastCar;
    static double distanceMetres(double lat1, double lon1, double lat2, double lon2);
    bool carInView(double lat, double lon, bool safeBorder) const;

    // --- File list (MainForm_FileList.cpp) ---
    void createFileListMenuButtons();
    void setupListView();
    void translateListView();
    void updateFileListMenu();  // FL-040…FL-047
    void clearListSelection();
    FileListView *listView() const;
    void createFileManager(bool autoSearch);
    void onFileManagerChanges();
    void addFiles(const QStringList &paths);
    bool isNowPlaying(int row) const;
    FileManager *fileManager = nullptr;
    void deleteRowFromDisk(int row);
    MovingInfo::Options trackOptions() const;
    void reloadCurrentItem();
    QString fillCoordinatesTemplate(QString t, const MovingValue &v) const;
    QString copyTemplateDefault() const;
    QPointer<FormAbout> aboutForm;
    QPointer<FormWhatsNew> whatsNewForm;
    QPointer<FormCropping> croppingForm;
    FormCropping *cropping();
    QPointer<FormRepair> repairForm;
    FormRepair *repairing();
    QPointer<FormGPSLog> gpsLogForm;
    FormGPSLog *gpsLog();
    QVector<gpsfiles::Source> selectionGps() const;  // FL-070
    QString lastSelectedName;  // FL-012
    QList<QPair<QToolButton *, QAction *>> fileListButtons;
};
