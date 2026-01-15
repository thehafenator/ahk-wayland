#include "activeclienteffect.h"
#include <kwin/workspace.h>
#include <kwin/window.h>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QTimer>

namespace KWin {

ActiveClientEffect::ActiveClientEffect()
{
    // Connect to window activation changes
    connect(workspace(), &Workspace::windowActivated,
            this, &ActiveClientEffect::onActiveClientChanged);
    
    // Connect to window creation
    connect(workspace(), &Workspace::windowAdded,
            this, &ActiveClientEffect::onWindowAdded);
    
    // Connect to window destruction
    connect(workspace(), &Workspace::windowRemoved,
            this, &ActiveClientEffect::onWindowRemoved);
    
    // Emit initial state for current window on startup
    QTimer::singleShot(0, this, &ActiveClientEffect::emitInitialState);
}

bool ActiveClientEffect::supported()
{
    return true;
}

void ActiveClientEffect::sendDBusSignal(const QString &signalName, const QString &windowClass, const QString &windowTitle)
{
    QDBusMessage msg = QDBusMessage::createSignal(
        QStringLiteral("/ActiveWindow"), 
        QStringLiteral("org.ahkwayland.ActiveWindow"), 
        signalName
    );
    
    msg << windowClass << windowTitle;
    QDBusConnection::sessionBus().send(msg);
}

bool ActiveClientEffect::hasProblematicTitle(const QString &title)
{
    return title.isEmpty() || title.startsWith(QStringLiteral("_"));
}

void ActiveClientEffect::onActiveClientChanged()
{
    auto *window = workspace()->activeWindow();
    if (window) {
        QString windowClass = window->resourceClass();
        QString windowTitle = window->caption();
        
        // Send immediate signal
        sendDBusSignal(QStringLiteral("Changed"), windowClass, windowTitle);
        
        // If title looks incomplete, start polling
        if (hasProblematicTitle(windowTitle) || windowTitle == windowClass) {
            m_lastActiveWindow = window;
            m_polledWindows.insert(window);
            retryActiveWindowTitle(1);
        } else {
            m_lastActiveWindow = nullptr;
        }
    }
}

void ActiveClientEffect::retryActiveWindowTitle(int attempt)
{
    if (!m_lastActiveWindow) {
        return;
    }
    
    // Check if this is still the active window
    if (m_lastActiveWindow != workspace()->activeWindow()) {
        m_polledWindows.remove(m_lastActiveWindow);
        m_lastActiveWindow = nullptr;
        return;
    }
    
    if (attempt > 3) {
        // After 3 quick attempts, start slower continuous polling
        QTimer::singleShot(500, this, &ActiveClientEffect::pollWindowTitle);
        return;
    }
    
    // Exponential backoff: 50ms, 150ms, 350ms
    int delay = 50 * (1 << (attempt - 1));
    
    QTimer::singleShot(delay, this, [this, attempt]() {
        if (!m_lastActiveWindow) return;
        
        QString windowClass = m_lastActiveWindow->resourceClass();
        QString windowTitle = m_lastActiveWindow->caption();
        
        // If we got a better title, send an update
        if (!hasProblematicTitle(windowTitle) && windowTitle != windowClass) {
            sendDBusSignal(QStringLiteral("Changed"), windowClass, windowTitle);
            m_polledWindows.remove(m_lastActiveWindow);
            m_lastActiveWindow = nullptr;
        } else {
            // Try next attempt
            retryActiveWindowTitle(attempt + 1);
        }
    });
}

void ActiveClientEffect::pollWindowTitle()
{
    auto *activeWindow = workspace()->activeWindow();
    
    // Clean up polled windows that no longer exist or aren't active
    auto it = m_polledWindows.begin();
    while (it != m_polledWindows.end()) {
        if (*it != activeWindow) {
            it = m_polledWindows.erase(it);
        } else {
            ++it;
        }
    }
    
    // If we have an active window being polled
    if (activeWindow && m_polledWindows.contains(activeWindow)) {
        QString windowClass = activeWindow->resourceClass();
        QString windowTitle = activeWindow->caption();
        
        // Check if title has improved
        if (!hasProblematicTitle(windowTitle) && windowTitle != windowClass) {
            sendDBusSignal(QStringLiteral("Changed"), windowClass, windowTitle);
            m_polledWindows.remove(activeWindow);
        } else {
            // Continue polling every 500ms
            QTimer::singleShot(500, this, &ActiveClientEffect::pollWindowTitle);
        }
    }
}

void ActiveClientEffect::onWindowAdded(Window *window)
{
    if (window) {
        QString windowClass = window->resourceClass();
        QString windowTitle = window->caption();
        sendDBusSignal(QStringLiteral("Created"), windowClass, windowTitle);
        
        // Listen for title changes on this window
        connect(window, &Window::captionChanged, this, [this, window]() {
            QString windowClass = window->resourceClass();
            QString windowTitle = window->caption();
            
            // Only send if this is the active window
            if (window == workspace()->activeWindow()) {
                // If we were polling this window and got a good title, stop polling
                if (m_polledWindows.contains(window) && !hasProblematicTitle(windowTitle)) {
                    m_polledWindows.remove(window);
                }
                sendDBusSignal(QStringLiteral("Changed"), windowClass, windowTitle);
            }
        });
    }
}

void ActiveClientEffect::onWindowRemoved(Window *window)
{
    if (window) {
        // Clean up from polled windows
        m_polledWindows.remove(window);
        if (m_lastActiveWindow == window) {
            m_lastActiveWindow = nullptr;
        }
        
        QString windowClass = window->resourceClass();
        QString windowTitle = window->caption();
        sendDBusSignal(QStringLiteral("Destroyed"), windowClass, windowTitle);
        
        // Immediately check what became active (no delay)
        QTimer::singleShot(0, this, &ActiveClientEffect::onActiveClientChanged);
    }
}
void ActiveClientEffect::emitInitialState()
{
    // Send the current active window on plugin startup
    if (auto *window = workspace()->activeWindow()) {
        QString windowClass = window->resourceClass();
        QString windowTitle = window->caption();
        sendDBusSignal(QStringLiteral("Initial"), windowClass, windowTitle);
    }
}

} // namespace KWin