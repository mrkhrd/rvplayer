#include "Dfm.h"

#include <QRegularExpression>

namespace vcl {

namespace {

struct Token {
    enum class Type { End, Ident, Number, String, Symbol };
    Type type = Type::End;
    QString text;
};

class Lexer {
public:
    explicit Lexer(const QString &text) : s(text) {}

    Token next() {
        skipSpace();
        if (pos >= s.size()) {
            return {};
        }
        const QChar c = s[pos];
        if (c.isLetter() || c == '_') {
            const qsizetype start = pos;
            while (pos < s.size() && (s[pos].isLetterOrNumber() || s[pos] == '_' || s[pos] == '.')) {
                ++pos;
            }
            return {Token::Type::Ident, s.mid(start, pos - start)};
        }
        if (c.isDigit() || ((c == '-' || c == '+') && pos + 1 < s.size() && (s[pos + 1].isDigit() || s[pos + 1] == '.'))) {
            const qsizetype start = pos++;
            while (pos < s.size() && (s[pos].isDigit() || s[pos] == '.' || s[pos] == 'e' || s[pos] == 'E' ||
                                      ((s[pos] == '-' || s[pos] == '+') && (s[pos - 1] == 'e' || s[pos - 1] == 'E')))) {
                ++pos;
            }
            return {Token::Type::Number, s.mid(start, pos - start)};
        }
        if (c == '\'') {
            QString out;
            ++pos;
            while (pos < s.size()) {
                if (s[pos] == '\'') {
                    if (pos + 1 < s.size() && s[pos + 1] == '\'') {
                        out += '\'';
                        pos += 2;
                        continue;
                    }
                    ++pos;
                    break;
                }
                out += s[pos++];
            }
            return {Token::Type::String, out};
        }
        ++pos;
        return {Token::Type::Symbol, QString(c)};
    }

    Token peek() {
        const qsizetype saved = pos;
        Token t = next();
        pos = saved;
        return t;
    }

    // Raw text up to (not including) the next `close` character, used for hex blocks.
    QString rawUntil(QChar close) {
        const qsizetype start = pos;
        while (pos < s.size() && s[pos] != close) {
            ++pos;
        }
        QString out = s.mid(start, pos - start);
        if (pos < s.size()) {
            ++pos;
        }
        return out;
    }

private:
    void skipSpace() {
        while (pos < s.size() && s[pos].isSpace()) {
            ++pos;
        }
    }

    const QString &s;
    qsizetype pos = 0;
};

class Parser {
public:
    explicit Parser(const QString &text) : lex(text) {}

    DfmObject parseObject() {
        DfmObject o;
        o.kind = expectIdent();
        o.name = expectIdent();
        if (lex.peek().text == ":") {
            lex.next();
            o.className = expectIdent();
        } else {
            o.className = o.name;  // anonymous objects: "object TClass"
            o.name.clear();
        }
        if (lex.peek().text == "[") {  // inherited child index: [n]
            lex.next();
            lex.next();
            lex.next();
        }
        while (!failed) {
            const Token t = lex.peek();
            if (t.type == Token::Type::End) {
                fail("unexpected end of file in " + o.name);
                break;
            }
            if (t.type == Token::Type::Ident && t.text.compare("end", Qt::CaseInsensitive) == 0) {
                lex.next();
                break;
            }
            if (t.type == Token::Type::Ident && (t.text == "object" || t.text == "inherited" || t.text == "inline")) {
                o.children.append(parseObject());
                continue;
            }
            const QString key = expectIdent();
            expectSymbol("=");
            o.props.append({key, parseValue()});
        }
        return o;
    }

    bool failed = false;
    QString error;

private:
    DfmValue parseValue() {
        DfmValue v;
        const Token t = lex.next();
        switch (t.type) {
        case Token::Type::String: {
            QString text = t.text;
            while (lex.peek().text == "+") {  // concatenated string literals
                lex.next();
                text += lex.next().text;
            }
            v.kind = DfmValue::Kind::String;
            v.scalar = text;
            return v;
        }
        case Token::Type::Number:
            if (t.text.contains('.') || t.text.contains('e') || t.text.contains('E')) {
                v.kind = DfmValue::Kind::Float;
                v.scalar = t.text.toDouble();
            } else {
                v.kind = DfmValue::Kind::Int;
                v.scalar = t.text.toLongLong();
            }
            return v;
        case Token::Type::Ident:
            v.kind = DfmValue::Kind::Ident;
            v.scalar = t.text;
            return v;
        case Token::Type::Symbol:
            break;
        case Token::Type::End:
            fail("unexpected end of value");
            return v;
        }
        if (t.text == "(") {
            v.kind = DfmValue::Kind::List;
            while (!failed && lex.peek().text != ")") {
                v.list.append(parseValue());
            }
            lex.next();
        } else if (t.text == "[") {
            v.kind = DfmValue::Kind::Set;
            while (!failed) {
                const Token e = lex.next();
                if (e.text == "]" || e.type == Token::Type::End) {
                    break;
                }
                if (e.type == Token::Type::Ident) {
                    v.set.append(e.text);
                }
            }
        } else if (t.text == "{") {
            v.kind = DfmValue::Kind::Binary;
            QString hex = lex.rawUntil('}');
            hex.remove(QRegularExpression(QStringLiteral("\\s")));
            v.binary = QByteArray::fromHex(hex.toLatin1());
        } else if (t.text == "<") {
            v.kind = DfmValue::Kind::Collection;
            while (!failed) {
                const Token e = lex.next();
                if (e.text == ">" || e.type == Token::Type::End) {
                    break;
                }
                if (e.type == Token::Type::Ident && e.text == "item") {
                    DfmItem item;
                    while (!failed) {
                        const Token p = lex.peek();
                        if (p.type == Token::Type::Ident && p.text == "end") {
                            lex.next();
                            break;
                        }
                        const QString key = expectIdent();
                        expectSymbol("=");
                        item.props.append({key, parseValue()});
                    }
                    v.items.append(item);
                }
                // an "[index]" before "item" or other symbols are skipped
            }
        } else if (t.text == "-") {  // negative number written with a space
            DfmValue inner = parseValue();
            inner.scalar = inner.kind == DfmValue::Kind::Int ? QVariant(-inner.scalar.toLongLong())
                                                             : QVariant(-inner.scalar.toDouble());
            return inner;
        } else {
            fail("unexpected symbol " + t.text);
        }
        return v;
    }

    QString expectIdent() {
        const Token t = lex.next();
        if (t.type != Token::Type::Ident) {
            fail("identifier expected, got '" + t.text + "'");
        }
        return t.text;
    }

    void expectSymbol(const QString &sym) {
        const Token t = lex.next();
        if (t.text != sym) {
            fail("'" + sym + "' expected, got '" + t.text + "'");
        }
    }

    void fail(const QString &msg) {
        if (!failed) {
            failed = true;
            error = msg;
        }
    }

    Lexer lex;
};

}  // namespace

qint64 DfmValue::toInt(qint64 fallback) const {
    if (kind == Kind::Int || kind == Kind::Float) {
        return scalar.toLongLong();
    }
    return fallback;
}

double DfmValue::toDouble(double fallback) const {
    if (kind == Kind::Int || kind == Kind::Float) {
        return scalar.toDouble();
    }
    return fallback;
}

QString DfmValue::toString() const {
    return (kind == Kind::String || kind == Kind::Ident) ? scalar.toString() : QString();
}

bool DfmValue::toBool(bool fallback) const {
    if (kind == Kind::Ident) {
        const QString s = scalar.toString();
        if (s.compare("True", Qt::CaseInsensitive) == 0) {
            return true;
        }
        if (s.compare("False", Qt::CaseInsensitive) == 0) {
            return false;
        }
    }
    return fallback;
}

QStringList DfmValue::toStringList() const {
    QStringList out;
    for (const DfmValue &v : list) {
        out.append(v.toString());
    }
    return out;
}

const DfmValue *DfmObject::prop(const QString &name) const {
    for (const auto &p : props) {
        if (p.first.compare(name, Qt::CaseInsensitive) == 0) {
            return &p.second;
        }
    }
    return nullptr;
}

qint64 DfmObject::intProp(const QString &name, qint64 fallback) const {
    const DfmValue *v = prop(name);
    return v ? v->toInt(fallback) : fallback;
}

QString DfmObject::stringProp(const QString &name, const QString &fallback) const {
    const DfmValue *v = prop(name);
    return v ? v->toString() : fallback;
}

bool DfmObject::boolProp(const QString &name, bool fallback) const {
    const DfmValue *v = prop(name);
    return v ? v->toBool(fallback) : fallback;
}

DfmObject parseDfm(const QString &text, QString *error) {
    Parser p(text);
    DfmObject root = p.parseObject();
    if (p.failed) {
        if (error) {
            *error = p.error;
        }
        return {};
    }
    return root;
}

}  // namespace vcl
