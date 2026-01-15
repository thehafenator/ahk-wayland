#include <QCoreApplication>
#include <QPluginLoader>
#include <QDebug>
#include <QDir>

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);

    // FIX: Use QStringLiteral to satisfy Qt 6 strict string matching
    QString pluginPath = QDir::homePath() + QStringLiteral("/.local/lib/plugins/kwin/effects/ahk-wayland-activeclient.so");
    
    qDebug() << "Attempting to load:" << pluginPath;
    
    QPluginLoader loader(pluginPath);
    
    if (loader.load()) {
        qDebug() << "SUCCESS! The plugin loaded correctly.";
        qDebug() << "Metadata:" << loader.metaData();
    } else {
        qDebug() << "FAILURE!";
        // This error string is what we need to see:
        qDebug() << "Error String:" << loader.errorString(); 
    }
    
    return 0;
}