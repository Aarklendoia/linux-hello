#include "VideoBridge.h"
#include <QVideoFrameFormat>
#include <QDebug>

// Instance globale pour l'accès depuis FFI
static VideoBridge *g_videoBridge = nullptr;

VideoBridge::VideoBridge(QObject *parent)
    : QObject(parent), m_videoSink(nullptr)
{
    g_videoBridge = this;
    register_video_bridge_callback();
}

VideoBridge::~VideoBridge()
{
    if (g_videoBridge == this)
        g_videoBridge = nullptr;
}

void VideoBridge::pushFrameYUYV(const uint8_t *data, size_t len, int width, int height)
{
    if (!m_videoSink)
        return;

    auto frame = createYUYVFrame(data, len, width, height);
    if (frame.isValid())
        m_videoSink->setVideoFrame(frame);
}

QVideoFrame VideoBridge::createYUYVFrame(const uint8_t *data, size_t len, int width, int height)
{
    QVideoFrameFormat format(
        QSize(width, height),
        QVideoFrameFormat::Format_YUYV // YUYV 4:2:2
    );

    // Créer un buffer à partir des données brutes
    QVideoFrame frame(format);

    // Mapper et copier les données
    if (frame.map(QVideoFrame::WriteOnly))
    {
        memcpy(frame.bits(0), data, len);
        frame.unmap();
    }

    return frame;
}

/// FFI : appelée depuis Rust pour envoyer une frame
extern "C"
{
    void push_frame_yuyv_from_rust(const uint8_t *data, size_t len, int width, int height)
    {
        if (g_videoBridge)
        {
            g_videoBridge->pushFrameYUYV(data, len, width, height);
        }
    }

    void register_video_bridge_callback()
    {
        // Enregistrer le callback dans Rust (optionnel, pour les statistiques)
        // video_bridge_set_callback(push_frame_yuyv_from_rust);
    }
}
