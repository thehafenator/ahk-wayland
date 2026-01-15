#ifndef ACTIVECLIENTEFFECT_H
#define ACTIVECLIENTEFFECT_H

#include <kwin/effect/effect.h>
#include <QSet>

namespace KWin {

class Window; // Forward declaration

class ActiveClientEffect : public Effect
{
    Q_OBJECT
public:
    ActiveClientEffect();
    ~ActiveClientEffect() override = default;

    static bool supported();
    static bool enabledByDefault() { return false; }

public Q_SLOTS:
    void onActiveClientChanged();
    void onWindowAdded(Window *window);
    void onWindowRemoved(Window *window);
    void emitInitialState();
    void retryActiveWindowTitle(int attempt);
    void pollWindowTitle();

private:
    void sendDBusSignal(const QString &signalName, const QString &windowClass, const QString &windowTitle);
    bool hasProblematicTitle(const QString &title);
    Window *m_lastActiveWindow = nullptr;
    QSet<Window*> m_polledWindows;
};

} // namespace KWin

#endif