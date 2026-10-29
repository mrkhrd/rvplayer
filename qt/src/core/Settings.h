// Application settings with the original value names and defaults (spec GM-001, ST-*), stored
// in the new app's own INI file rather than the original's registry key (decisions.md D-2).
#pragma once

#include <QString>

class QSettings;

class Settings {
public:
    static Settings &instance();

    bool readBool(const QString &name, bool def) const;
    int readInt(const QString &name, int def) const;
    double readFloat(const QString &name, double def) const;
    QString readString(const QString &name, const QString &def = {}) const;

    void writeBool(const QString &name, bool v);
    void writeInt(const QString &name, int v);
    void writeFloat(const QString &name, double v);  // 3 decimals, as the original
    void writeString(const QString &name, const QString &v);
    void remove(const QString &name);
    bool contains(const QString &name) const;
    // SH-024: every value is removed.
    void clear();

private:
    Settings();
    QSettings *store;
};
