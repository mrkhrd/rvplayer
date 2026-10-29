// TFormDetails: the borderless popup with the values at the timeline position under the mouse
// (hover over the seek bar or a graph). Spec: graphs-map-layout.md §3.10 (GM-120…GM-123).
#pragma once

#include "core/MovingInfo.h"
#include "draw/Graphics.h"
#include "vcl/Form.h"

#include <optional>

class FormDetails : public vcl::Form {
    Q_OBJECT
public:
    enum class Profile { GpsG, Gps, Video };

    struct Content {
        QString file;            // sub-file name, "[k] name" in a group
        QString systemTime, videoTime;
        MovingValue value;
        bool british = false;
        Profile profile = Profile::GpsG;
        // Mini-graph: the graph's kind and its columns around the position (GM-122).
        std::optional<draw::Graphic::Kind> graph;
        QString graphName;
        QVector<draw::Bin> bins;
        int cursorColumn = -1;   // play position inside the mini-graph, -1 outside
    };

    explicit FormDetails(QWidget *parent = nullptr);

    // GM-122/GM-123: fills the popup and shows it near the cursor without taking focus.
    void popup(const Content &c, QPoint cursorGlobal, int panelTopGlobal, int panelBottomGlobal);
    void popdown();
    // Each timer tick only the mini-graph cursor moves (TimerTimer L1359).
    void drawCursor(int column);

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormMouseMove(vcl::Event *e);

protected:
    void mouseMoveEvent(QMouseEvent *e) override;

private:
    void applyProfile(Profile p);
    void setLabel(const char *name, const QString &text);
    void alignCaptions();
    draw::Graphic graphic{draw::Graphic::Kind::S};
    bool hasGraph = false;
};
