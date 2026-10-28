#include "Settings.h"

#include <QSettings>

Settings &Settings::instance() {
    static Settings s;
    return s;
}

Settings::Settings() : store(new QSettings(QSettings::IniFormat, QSettings::UserScope, "RVPlayer", "rvplayer")) {}

bool Settings::readBool(const QString &name, bool def) const {
    const QString v = store->value(name).toString();
    if (v == "true") return true;
    if (v == "false") return false;
    return def;  // anything else reads as the default (GM-001)
}

int Settings::readInt(const QString &name, int def) const {
    bool ok = false;
    const int v = store->value(name).toString().toInt(&ok);
    return ok ? v : def;
}

double Settings::readFloat(const QString &name, double def) const {
    bool ok = false;
    const double v = store->value(name).toString().toDouble(&ok);
    return ok ? v : def;
}

QString Settings::readString(const QString &name, const QString &def) const {
    return store->contains(name) ? store->value(name).toString() : def;
}

void Settings::writeBool(const QString &name, bool v) {
    store->setValue(name, v ? "true" : "false");
}

void Settings::writeInt(const QString &name, int v) {
    store->setValue(name, QString::number(v));
}

void Settings::writeFloat(const QString &name, double v) {
    store->setValue(name, QString::number(v, 'f', 3));
}

void Settings::writeString(const QString &name, const QString &v) {
    store->setValue(name, v);
}

void Settings::remove(const QString &name) {
    store->remove(name);
}

bool Settings::contains(const QString &name) const {
    return store->contains(name);
}
