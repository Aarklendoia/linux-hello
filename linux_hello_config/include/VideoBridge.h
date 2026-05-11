#pragma once

#include <QObject>
#include <QVideoSink>
#include <QVideoFrame>
#include <QSize>
#include <memory>

/// Pont Qt6/C++ pour afficher le flux vidéo V4L2 dans QML
/// Reçoit les frames brutes YUYV depuis Rust et les convertit en QVideoFrame
class VideoBridge : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVideoSink *videoSink READ videoSink WRITE setVideoSink NOTIFY videoSinkChanged)

public:
    explicit VideoBridge(QObject *parent = nullptr);
    ~VideoBridge();

    /// Obtenir le QVideoSink à passer à VideoOutput en QML
    QVideoSink *videoSink() const { return m_videoSink; }

    /// Définir le QVideoSink
    void setVideoSink(QVideoSink *sink)
    {
        if (m_videoSink == sink)
            return;
        m_videoSink = sink;
        emit videoSinkChanged();
    }

    /// Recevoir une frame YUYV depuis Rust
    /// Appelée via FFI : push_frame_yuyv_from_rust()
    void pushFrameYUYV(const uint8_t *data, size_t len, int width, int height);

signals:
    void videoSinkChanged();

private:
    QVideoSink *m_videoSink = nullptr;
    QSize m_lastSize;

    /// Créer un QVideoFrame à partir de données YUYV
    QVideoFrame createYUYVFrame(const uint8_t *data, size_t len, int width, int height);
};

/// Fonction FFI appelée depuis Rust
extern "C"
{
    void push_frame_yuyv_from_rust(const uint8_t *data, size_t len, int width, int height);
    void register_video_bridge_callback();
}
