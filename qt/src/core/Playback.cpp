#include "Playback.h"

#include "core/Trace.h"
#include "rv_ffi.h"

#include <algorithm>
#include <cstdio>
#include <thread>
#include <cmath>

namespace {

struct FrameBuffer {
    uint8_t *data;
    size_t len;
};

void releaseFrame(void *info) {
    auto *b = static_cast<FrameBuffer *>(info);
    rv_frame_free(b->data, b->len);
    delete b;
}

// The decoder's RGBA buffer as an image without copying; freed with the image.
QImage wrapFrame(RvEvent &e) {
    auto *b = new FrameBuffer{e.rgba, e.rgba_len};
    return QImage(e.rgba, int(e.width), int(e.height), int(e.width) * 4, QImage::Format_RGBA8888, releaseFrame, b);
}

}  // namespace

Playback::Playback(QObject *parent) : QObject(parent) {
    pumpTimer.setTimerType(Qt::PreciseTimer);
    pumpTimer.setInterval(4);
    connect(&pumpTimer, &QTimer::timeout, this, &Playback::pump);
}

Playback::~Playback() {
    closePlayer();
}

void Playback::setGroup(const Group &g) {
    unload();
    group_ = g;
    current = g.isEmpty() ? -1 : 0;
    lastSeekPos = 0;
}

void Playback::closePlayer() {
    RV_TRACE_SCOPE("Playback::closePlayer");
    pumpTimer.stop();
    // Closing waits for the decoding thread to finish its current frame; that wait must not
    // freeze the window when another file is chosen.
    if (player) std::thread([p = player] { rv_player_close(p); }).detach();
    player = nullptr;
    queue.clear();
    shownTime = newestTime = -1;
    endOfStream = false;
}

void Playback::unload() {
    closePlayer();
    state_ = State::Stopped;
}

void Playback::openFile(int index, double seconds) {
    RV_TRACE_SCOPE("Playback::openFile");
    closePlayer();
    current = index;
    const GroupFile &f = group_.files[index];
    duration = f.duration * 86400.0;
    player = rv_player_open(f.path.toUtf8().constData(), std::max(0.0, seconds));
    clockBase = seconds;
    waitFirst = true;
    seekLatency.start();
    if (player) pumpTimer.start();
}

void Playback::restartDecoder(double seconds) {
    if (!player) return;
    rv_player_seek(player, std::max(0.0, seconds));
    queue.clear();
    newestTime = -1;
    endOfStream = false;
    clockBase = seconds;
    waitFirst = true;
    seekLatency.start();
}

double Playback::seek(double p, bool force) {
    RV_TRACE_SCOPE("Playback::seek");
    ++seekCount;
    lastSeekPos = p;
    if (group_.isEmpty() || p < 0 || p > 1) return -1;
    const int k = group_.fileAt(p);
    if (p < group_.files[k].pStart) p = group_.files[k].pStart;
    const double local = std::clamp(group_.localAt(k, p), 0.0, 1.0);
    const double seconds = local * group_.files[k].duration * 86400.0;
    if (k != current || !player) {
        if (player || force) {
            if (state_ == State::Stopped) state_ = State::Paused;
            openFile(k, seconds);
        } else {
            current = k;
        }
    } else {
        restartDecoder(seconds);
    }
    lastSeekPos = p;
    return p;
}

void Playback::play() {
    if (!player || state_ == State::Running) return;
    state_ = State::Running;
    if (shownTime >= 0 && !waitFirst) clockBase = shownTime;
    clock.restart();
}

void Playback::pause() {
    if (state_ != State::Running) return;
    clockBase = clockSeconds();
    state_ = State::Paused;
}

void Playback::setRate(double rate) {
    if (state_ == State::Running) {
        clockBase = clockSeconds();
        clock.restart();
    }
    rate_ = rate;
}

double Playback::clockSeconds() const {
    if (state_ != State::Running || waitFirst || !clock.isValid()) return clockBase;
    return clockBase + clock.elapsed() / 1000.0 * rate_;
}

double Playback::fileSeconds() const {
    return duration > 0 ? duration : (current >= 0 ? group_.files[current].duration * 86400.0 : 0);
}

double Playback::positionOf(double seconds) const {
    if (current < 0) return lastSeekPos;
    const double d = fileSeconds();
    const double local = d > 0 ? std::clamp(seconds / d, 0.0, 1.0) : 0;
    return group_.globalAt(current, local);
}

double Playback::position() {
    if (!player || current < 0) return lastSeekPos;
    const double t = state_ == State::Running ? clockSeconds() : (shownTime >= 0 ? shownTime : clockBase);
    // PV-005 step 4: at the end of a file the group continues with the next one, gaps skipped.
    const bool atEnd = (endOfStream && queue.empty() && t >= fileSeconds() - 0.05) ||
                       (trimTail > 0 && t >= fileSeconds() - trimTail);
    if (state_ == State::Running && atEnd && current + 1 < group_.files.size()) {
        const double next = std::max(group_.files[current + 1].pStart, group_.files[current].pFinish);
        seek(next);
        return lastSeekPos;
    }
    return positionOf(std::min(t, fileSeconds()));
}

void Playback::show(Decoded &&d) {
    shownTime = d.time;
    emit frameReady(d.image);
}

void Playback::stepForward() {
    if (!player) return;
    pause();
    if (!queue.empty()) {
        Decoded d = std::move(queue.front());
        queue.pop_front();
        clockBase = d.time;
        show(std::move(d));
    } else if (!endOfStream) {
        stepPending = true;
    }
}

void Playback::stepBack() {
    if (!player) return;
    pause();
    const double target = (shownTime >= 0 ? shownTime : clockBase) - 0.5 / std::max(1.0, frameRate);
    if (target < 0) return;
    restartDecoder(target);
}

void Playback::pump() {
    RV_TRACE_SCOPE("Playback::pump");
    if (!player) return;
    const uint64_t generation = rv_player_generation(player);
    RvEvent e;
    while (queue.size() < 8 && rv_player_poll(player, &e)) {
        switch (e.kind) {
        case RV_EVENT_OPENED:
            duration = e.duration;
            frameRate = e.frame_rate > 0 ? e.frame_rate : 30;
            emit fileOpened(current, QSize(int(e.width), int(e.height)));
            break;
        case RV_EVENT_FRAME: {
            QImage img = wrapFrame(e);
            if (e.generation != generation) break;  // decoded before the last seek
            newestTime = e.time;
            queue.push_back({std::move(img), e.time});
            break;
        }
        case RV_EVENT_END_OF_STREAM:
            if (e.generation == generation) endOfStream = true;
            break;
        case RV_EVENT_ERROR:
            if (e.error) {
                qWarning("video: %s", e.error);
                rv_string_free(e.error);
            }
            endOfStream = true;
            break;
        default:
            break;
        }
    }

    // After a seek the first frame is shown at once; a running clock starts from it.
    if (waitFirst) {
        if (queue.empty()) return;
        Decoded d = std::move(queue.front());
        queue.pop_front();
        waitFirst = false;
        clockBase = d.time;
        clock.restart();
        if (TraceScope::enabled()) std::fprintf(stderr, "seek: first frame after %lld ms\n", seekLatency.elapsed());
        show(std::move(d));
        return;
    }
    if (stepPending && !queue.empty()) {
        stepPending = false;
        Decoded d = std::move(queue.front());
        queue.pop_front();
        clockBase = d.time;
        show(std::move(d));
        return;
    }
    if (state_ != State::Running) return;

    // The newest frame whose time has come; older ones are skipped.
    const double now = clockSeconds();
    bool have = false;
    Decoded due;
    while (!queue.empty() && queue.front().time <= now) {
        due = std::move(queue.front());
        queue.pop_front();
        have = true;
    }
    if (have) show(std::move(due));

    // Faster than the decoder (high play speeds): jump ahead instead of falling behind.
    const double lag = now - std::max(newestTime, shownTime);
    if (!endOfStream && queue.empty() && newestTime >= 0 && lag > std::max(1.0, 0.5 * rate_)) {
        const double ahead = std::min(now + 0.25 * rate_, fileSeconds());
        restartDecoder(ahead);
        clockBase = now;
        waitFirst = false;
        clock.restart();
    }
}
