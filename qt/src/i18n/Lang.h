// UI strings from the original translation files (spec: shell-settings-forms.md §4, TR-001…TR-041).
#pragma once

#include <QColor>
#include <QHash>
#include <QString>
#include <QStringList>
#include <QVector>

class Lang {
public:
    static Lang &instance();

    // Loads the bundled languages (English and Russian) and selects the initial one.
    void load();

    QStringList ids() const;                 // "en", "ru"
    QString nativeName(const QString &id) const;
    QString current() const { return currentId; }
    void setCurrent(const QString &id);

    // Lookup with fallback: current language, then English, then the key itself (TR-008).
    QString tr(const QString &key) const;
    QString trOr(const QString &key, const QString &backupKey) const;
    QString trDef(const QString &key, const QString &def) const;
    bool has(const QString &key) const;

    // Colour entries (`#RRGGBB` or `clName`); missing or invalid ones are red (TR-040).
    QColor color(const QString &key) const;
    // A Delphi colour value: `#RRGGBB`, a system colour (`clBtnFace`, ...) or a named one.
    static QColor vclColor(const QString &value);

private:
    struct Language {
        QString id;
        QString name;
        QStringList systemIds;
        QHash<QString, QString> values;  // lower-cased key → value
    };
    const QString *find(const Language &l, const QString &key) const;
    const Language *byId(const QString &id) const;

    QVector<Language> languages;
    QString currentId = "en";
};

inline QString tr_(const QString &key) {
    return Lang::instance().tr(key);
}
