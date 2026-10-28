#include "Lang.h"

#include <QApplication>
#include <QFile>
#include <QPalette>
#include <QRegularExpression>

#ifdef Q_OS_WIN
#include <windows.h>
#endif

namespace {

QString readResource(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return {};
    QString text = QString::fromUtf8(f.readAll());
    if (text.startsWith(QChar(0xFEFF))) text.remove(0, 1);  // TR-002
    return text;
}

// TR-003: one "key value" pair per non-empty line; keys are case-insensitive; first one wins.
void parseInto(const QString &text, QHash<QString, QString> &out) {
    static const QRegularExpression lines(QStringLiteral("[\r\n]+"));
    for (QString line : text.split(lines, Qt::SkipEmptyParts)) {
        line = line.trimmed();
        qsizetype i = 0;
        while (i < line.size() && line[i] != ' ' && line[i] != '\t') ++i;
        const QString key = line.left(i).toLower();
        QString value = line.mid(i).trimmed();
        if (value.startsWith('"')) value = value.mid(1, value.size() - 2);
        value.replace(QStringLiteral("\\n"), QStringLiteral("\r\n"));
        if (key.isEmpty() || value.isEmpty() || out.contains(key)) continue;
        out.insert(key, value);
    }
}

}  // namespace

Lang &Lang::instance() {
    static Lang lang;
    return lang;
}

void Lang::load() {
    languages.clear();
    const QString shared = readResource(":/original/lang/TRANSLATE_ALL.TXT");
    for (const QString &code : {QStringLiteral("EN"), QStringLiteral("RU")}) {
        Language l;
        // TR-007 merge order: the brand's fix file, language file, shared file. Of the fix file
        // only the interface fixes are kept, not the DATAKAM contact entries (My*).
        QHash<QString, QString> fixes;
        parseInto(readResource(":/original/lang/TRANSLATE_FIX_" + code + ".TXT"), fixes);
        for (auto it = fixes.cbegin(); it != fixes.cend(); ++it) {
            if (!it.key().startsWith(QStringLiteral("my"))) l.values.insert(it.key(), it.value());
        }
        parseInto(readResource(":/original/lang/TRANSLATE_" + code + ".TXT"), l.values);
        parseInto(shared, l.values);
        l.id = l.values.value("langid");
        if (l.id.isEmpty()) continue;  // TR-004: a file without LangID is discarded
        l.name = l.values.value("langname", l.id);
        for (const QString &s : l.values.value("langsystemid").split(',', Qt::SkipEmptyParts)) {
            l.systemIds.append(s.trimmed());
        }
        languages.append(l);
    }
    // TR-005: values of the exact form %OtherKey% are replaced once at load time.
    static const QRegularExpression ref(QStringLiteral("^%([^%]+)%$"));
    for (Language &l : languages) {
        for (auto it = l.values.begin(); it != l.values.end(); ++it) {
            const auto m = ref.match(it.value());
            if (!m.hasMatch()) continue;
            const QString target = m.captured(1).toLower();
            if (const QString *v = find(l, target)) it.value() = *v;
            else if (const Language *en = byId("en"); en && en != &l && find(*en, target)) it.value() = *find(*en, target);
        }
    }
    // TR-009: initial language from the system language id.
    currentId = "en";
#ifdef Q_OS_WIN
    const QString sys = QString::number(GetSystemDefaultLangID());
    for (const Language &l : languages) {
        if (l.systemIds.contains(sys)) {
            currentId = l.id;
            break;
        }
    }
#endif
}

QStringList Lang::ids() const {
    QStringList out;
    for (const Language &l : languages) out.append(l.id);
    return out;
}

QString Lang::nativeName(const QString &id) const {
    const Language *l = byId(id);
    return l ? l->name : id;
}

void Lang::setCurrent(const QString &id) {
    if (byId(id)) currentId = id;
}

const Lang::Language *Lang::byId(const QString &id) const {
    for (const Language &l : languages) {
        if (l.id == id) return &l;
    }
    return nullptr;
}

const QString *Lang::find(const Language &l, const QString &key) const {
    auto it = l.values.constFind(key.toLower());
    return it == l.values.constEnd() ? nullptr : &it.value();
}

QString Lang::trDef(const QString &key, const QString &def) const {
    if (const Language *cur = byId(currentId)) {
        if (const QString *v = find(*cur, key)) return *v;
    }
    if (currentId != "en") {
        if (const Language *en = byId("en")) {
            if (const QString *v = find(*en, key)) return *v;
        }
    }
    return def;
}

QString Lang::tr(const QString &key) const {
    return trDef(key, key);
}

QString Lang::trOr(const QString &key, const QString &backupKey) const {
    if (const Language *cur = byId(currentId)) {
        if (const QString *v = find(*cur, key)) return *v;
        if (const QString *v = find(*cur, backupKey)) return *v;
    }
    return trDef(key, key);
}

bool Lang::has(const QString &key) const {
    return trDef(key, QString()).size() > 0;
}

QColor Lang::color(const QString &key) const {
    return vclColor(trDef(key, QString()));
}

QColor Lang::vclColor(const QString &value) {
    const QString v = value.trimmed();
    if (v.startsWith('#')) {
        QColor c(v);
        if (c.isValid()) return c;
    }
    // VCL system colours are the Windows system colours (GetSysColor), not Qt's palette, which
    // follows the accent colour.
#ifdef Q_OS_WIN
    static const QHash<QString, int> sys = {
        {"clbtnface", COLOR_BTNFACE},     {"clwindow", COLOR_WINDOW},         {"clwindowtext", COLOR_WINDOWTEXT},
        {"clhighlight", COLOR_HIGHLIGHT}, {"clhighlighttext", COLOR_HIGHLIGHTTEXT}, {"clbtntext", COLOR_BTNTEXT},
        {"clbtnshadow", COLOR_BTNSHADOW}, {"clbtnhighlight", COLOR_BTNHIGHLIGHT}, {"clinfobk", COLOR_INFOBK},
    };
    if (auto it = sys.constFind(v.toLower()); it != sys.constEnd()) {
        const DWORD c = GetSysColor(it.value());
        return QColor(GetRValue(c), GetGValue(c), GetBValue(c));
    }
#else
    const QPalette pal = QApplication::palette();
    static const QHash<QString, QPalette::ColorRole> system = {
        {"clbtnface", QPalette::Button},          {"clwindow", QPalette::Base},
        {"clwindowtext", QPalette::Text},         {"clhighlight", QPalette::Highlight},
        {"clhighlighttext", QPalette::HighlightedText}, {"clbtntext", QPalette::ButtonText},
        {"clbtnshadow", QPalette::Dark},          {"clbtnhighlight", QPalette::Light},
        {"clinfobk", QPalette::ToolTipBase},
    };
    if (auto it = system.constFind(v.toLower()); it != system.constEnd()) return pal.color(it.value());
#endif
    static const QHash<QString, QColor> named = {
        {"clblack", Qt::black}, {"clwhite", Qt::white}, {"clred", Qt::red}, {"clblue", Qt::blue},
        {"clgreen", QColor(0, 128, 0)}, {"clgray", Qt::gray}, {"clsilver", QColor(192, 192, 192)},
        {"clyellow", Qt::yellow}, {"clnavy", QColor(0, 0, 128)}, {"clmaroon", QColor(128, 0, 0)},
    };
    return named.value(v.toLower(), Qt::red);
}
