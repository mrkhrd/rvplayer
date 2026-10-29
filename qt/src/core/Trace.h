// Slow-operation tracing for finding UI stalls: with the environment variable RV_TRACE set, every
// traced scope that takes longer than 15 ms is reported on stderr.
#pragma once

#include <QElapsedTimer>

#include <cstdio>

class TraceScope {
public:
    explicit TraceScope(const char *name) : name(name) {
        if (enabled()) timer.start();
    }
    ~TraceScope() {
        if (!timer.isValid()) return;
        const double ms = timer.nsecsElapsed() / 1e6;
        if (ms > 15) {
            std::fprintf(stderr, "slow: %s %.0f ms\n", name, ms);
            std::fflush(stderr);
        }
    }
    static bool enabled() {
        static const bool on = qEnvironmentVariableIsSet("RV_TRACE");
        return on;
    }

private:
    const char *name;
    QElapsedTimer timer;
};

#define RV_TRACE_CONCAT2(a, b) a##b
#define RV_TRACE_CONCAT(a, b) RV_TRACE_CONCAT2(a, b)
#define RV_TRACE_SCOPE(name) TraceScope RV_TRACE_CONCAT(rvTraceScope, __LINE__)(name)
