// Reader for Delphi form files in text format (.dfm): objects, properties and values.
#pragma once

#include <QList>
#include <QString>
#include <QStringList>
#include <QVariant>

namespace vcl {

struct DfmItem;

struct DfmValue {
    enum class Kind { Nil, Int, Float, String, Ident, Set, List, Binary, Collection };
    Kind kind = Kind::Nil;
    QVariant scalar;              // Int (qint64), Float (double), String / Ident (QString)
    QStringList set;              // Set: identifiers
    QList<DfmValue> list;         // List: values
    QByteArray binary;            // Binary
    QList<DfmItem> items;         // Collection

    bool isNull() const { return kind == Kind::Nil; }
    qint64 toInt(qint64 fallback = 0) const;
    double toDouble(double fallback = 0) const;
    QString toString() const;     // String or Ident text
    bool toBool(bool fallback = false) const;
    QStringList toStringList() const;  // List of strings (e.g. Items.Strings)
};

using DfmProperties = QList<QPair<QString, DfmValue>>;

struct DfmItem {
    DfmProperties props;
};

struct DfmObject {
    QString kind;                 // object / inherited / inline
    QString name;
    QString className;
    DfmProperties props;
    QList<DfmObject> children;

    const DfmValue *prop(const QString &name) const;
    qint64 intProp(const QString &name, qint64 fallback = 0) const;
    QString stringProp(const QString &name, const QString &fallback = {}) const;
    bool boolProp(const QString &name, bool fallback = false) const;
    bool has(const QString &name) const { return prop(name) != nullptr; }
};

// Parses a text .dfm. On failure returns an empty object and sets `error`.
DfmObject parseDfm(const QString &text, QString *error = nullptr);

}  // namespace vcl
