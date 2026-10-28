// Playback of the current group: one Rust decoder (rv_player) for the file under the position,
// frames shown when the media clock reaches them, files of a group played one after the other.
// It replaces the original's DirectShow graph; the position rules are the spec's.
// Spec: playback-video.md PV-001…PV-007, PV-018, PV-052; decisions.md Q-6, Q-8.
#pragma once

#include "Group.h"

#include <QElapsedTimer>
#include <QImage>
#include <QObject>
#include <QTimer>

#include <deque>

struct RvPlayer;

class Playback : public QObject {
    Q_OBJECT
public:
    enum class State { Stopped, Paused, Running };

    explicit Playback(QObject *parent = nullptr);
    ~Playback() override;

    // PV-010: the item to play; nothing is opened yet.
    void setGroup(const Group &g);
    const Group &group() const { return group_; }
    // A file is open (the original's "player active").
    bool active() const { return player != nullptr; }
    State state() const { return state_; }
    int fileIndex() const { return current; }

    void play();
    void pause();
    // Stops and releases the decoder (Stop button).
    void unload();

    // PV-006. `force` opens the file even when nothing is open yet (PV-013). Returns the position
    // actually used (clamped out of a gap), or -1 when the call is ignored.
    double seek(double p, bool force = false);
    // PV-005 without the pin drag: the position of the media clock, including the move to the
    // next file of the group at a file's end.
    double position();

    // PV-052 / Q-6: the clock runs at `rate` times real time.
    void setRate(double rate);
    double rate() const { return rate_; }

    // Q-8: true single-frame steps while paused.
    void stepForward();
    void stepBack();

signals:
    void frameReady(const QImage &frame);
    // A file was opened; size is the picture size.
    void fileOpened(int index, QSize size);

private:
    struct Decoded {
        QImage image;
        double time = 0;  // seconds in the file
    };
    void openFile(int index, double seconds);
    void closePlayer();
    void restartDecoder(double seconds);
    void pump();
    void show(Decoded &&d);
    double clockSeconds() const;
    double fileSeconds() const;
    double positionOf(double seconds) const;

    Group group_;
    RvPlayer *player = nullptr;
    int current = -1;
    State state_ = State::Stopped;
    double lastSeekPos = 0;
    double rate_ = 1;

    QTimer pumpTimer;
    QElapsedTimer clock;
    double clockBase = 0;         // media seconds when `clock` was started
    bool waitFirst = false;       // after a seek: the clock starts with the first frame
    bool stepPending = false;     // show the next decoded frame while paused
    std::deque<Decoded> queue;    // decoded, not yet shown
    double shownTime = -1;        // media time of the frame on screen
    double newestTime = -1;       // newest decoded frame
    double duration = 0;          // seconds of the open file
    double frameRate = 30;
    bool endOfStream = false;
};
