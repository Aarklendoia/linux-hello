//! Linux Hello - Configuration GUI pour KDE/Wayland
//!
//! Interface graphique pour:
//! - Enregistrement de visage avec preview en direct
//! - Configuration des paramètres d'authentification
//! - Gestion des visages enregistrés

#[allow(unused_imports)]
use iced::widget::{Button, Column, Container, ProgressBar, Row, Text};
use iced::{executor, Application, Command, Element, Length};
use std::time::Instant;

mod config;
mod dbus_client;
mod preview;
mod streaming;
mod ui;

use streaming::CaptureFrame;
use ui::Screen;

pub fn main() -> iced::Result {
    LinuxHelloConfig::run(Default::default())
}

/// Application principale
struct LinuxHelloConfig {
    current_screen: Screen,
    current_frame: Option<CaptureFrame>,
    frame_count: u32,
    total_frames: u32,
    capture_active: bool,
    preview_state: preview::PreviewState,

    // Animation state
    animated_progress: f32,         // Animated version of progress_percent()
    progress_animation_target: f32, // Target progress value
    last_animation_update: Instant, // Track timing for smooth animation
    animation_preview_opacity: f32, // Fade-in effect for preview area (0.0-1.0)
}

#[derive(Debug, Clone)]
enum Message {
    // Navigation
    GoToHome,
    GoToEnroll,
    GoToSettings,
    GoToManageFaces,

    // Enrollment
    StartCapture,
    StopCapture,
    FrameCaptured(Vec<u8>),

    // D-Bus Streaming
    CaptureProgressReceived(String), // JSON event from daemon
    CaptureCompleted(u32),           // user_id
    CaptureError(String),            // error message

    // Settings
    SettingChanged(String, String),

    // Animations
    AnimationTick, // Update animations every frame

    // General
    WindowClosed,
}

impl Application for LinuxHelloConfig {
    type Message = Message;
    type Executor = executor::Default;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                current_screen: Screen::Home,
                current_frame: None,
                frame_count: 0,
                total_frames: 0,
                capture_active: false,
                preview_state: preview::PreviewState::new(),
                animated_progress: 0.0,
                progress_animation_target: 0.0,
                last_animation_update: Instant::now(),
                animation_preview_opacity: 1.0,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Linux Hello - Configuration".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::GoToHome => {
                self.current_screen = Screen::Home;
                self.capture_active = false;
            }
            Message::GoToEnroll => {
                self.current_screen = Screen::Enrollment;
            }
            Message::GoToSettings => {
                self.current_screen = Screen::Settings;
            }
            Message::GoToManageFaces => {
                self.current_screen = Screen::ManageFaces;
            }
            Message::StartCapture => {
                self.capture_active = true;
                self.frame_count = 0;
                self.total_frames = 30;
                self.animation_preview_opacity = 0.5; // Fade-in start
                                                      // TODO: Lancer la capture via D-Bus
            }
            Message::StopCapture => {
                self.capture_active = false;
                self.animation_preview_opacity = 1.0; // Reset opacity
                                                      // TODO: Arrêter la capture
            }
            Message::FrameCaptured(_data) => {
                // TODO: Afficher la frame
            }
            Message::CaptureProgressReceived(json) => {
                // Parser le JSON et mettre à jour current_frame
                if let Ok(frame) = serde_json::from_str::<CaptureFrame>(&json) {
                    self.frame_count = frame.frame_number + 1;
                    self.total_frames = frame.total_frames;

                    // Update animation target with new progress
                    self.progress_animation_target =
                        (frame.frame_number as f32 + 1.0) / frame.total_frames as f32;

                    self.current_frame = Some(frame.clone());
                    self.preview_state.update_frame(frame);

                    // Fade-in preview
                    if self.animation_preview_opacity < 1.0 {
                        self.animation_preview_opacity =
                            (self.animation_preview_opacity + 0.15).min(1.0);
                    }
                }
            }
            Message::CaptureCompleted(user_id) => {
                tracing::info!("Capture complétée pour user_id={}", user_id);
                self.capture_active = false;
                self.animated_progress = 1.0;
            }
            Message::CaptureError(err) => {
                tracing::error!("Erreur capture: {}", err);
                self.capture_active = false;
            }
            Message::SettingChanged(_key, _value) => {
                // TODO: Sauvegarder le paramètre
            }
            Message::AnimationTick => {
                // Smooth animation towards target
                const ANIMATION_DURATION: f32 = 300.0; // ms

                let now = Instant::now();
                let elapsed = now.duration_since(self.last_animation_update).as_secs_f32() * 1000.0; // Convert to ms

                // Interpolate progress towards target
                if (self.animated_progress - self.progress_animation_target).abs() > 0.001 {
                    let delta = self.progress_animation_target - self.animated_progress;
                    let speed = (elapsed / ANIMATION_DURATION).min(1.0);
                    self.animated_progress += delta * speed * 0.1; // Smooth factor
                    self.animated_progress = self.animated_progress.max(0.0).min(1.0);
                }

                self.last_animation_update = now;
            }
            Message::WindowClosed => {
                // TODO: Cleanup
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let content = match self.current_screen {
            Screen::Home => self.view_home(),
            Screen::Enrollment => self.view_enrollment(),
            Screen::Settings => self.view_settings(),
            Screen::ManageFaces => self.view_manage_faces(),
        };

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        // TODO: S'abonner aux signaux D-Bus pour les frames
        // TODO: Implémenter les ticks d'animation via tokio
        iced::Subscription::none()
    }
}

impl LinuxHelloConfig {
    fn view_home(&self) -> Element<Message> {
        Row::new()
            .push(
                Container::new(Text::new("Accueil"))
                    .width(Length::Fill)
                    .center_x(),
            )
            .into()
    }

    fn view_enrollment(&self) -> Element<'_, Message> {
        use iced::widget::{Button, Column, ProgressBar};

        let progress = self.animated_progress; // Use animated progress instead
        let progress_text = self.preview_state.progress_text();
        let detection_text = self.preview_state.detection_status();
        let animation_opacity = self.animation_preview_opacity; // Capture value

        let preview_display = if self.preview_state.current_frame.is_some() {
            // Afficher: "Frame en cours de capture" avec fade-in
            Container::new(
                Column::new()
                    .push(Text::new("📹 Preview en direct"))
                    .push(Text::new(format!(
                        "Résolution: {}×{}",
                        self.preview_state.width, self.preview_state.height
                    )))
                    .push(Text::new(detection_text))
                    .spacing(10),
            )
            .width(Length::Fill)
            .padding(20)
            .style(move |_theme: &iced::Theme| {
                use iced::widget::container;

                // Dynamic opacity based on animation state
                let bg_color = iced::Color::from_rgb(0.1, 0.1, 0.1);
                let rgba = iced::Color {
                    r: bg_color.r,
                    g: bg_color.g,
                    b: bg_color.b,
                    a: animation_opacity, // Apply fade animation
                };

                container::Appearance {
                    background: Some(rgba.into()),
                    ..Default::default()
                }
            })
        } else {
            Container::new(Text::new("En attente de capture...").size(16))
                .width(Length::Fill)
                .padding(40)
                .center_x()
        };

        let progress_bar = ProgressBar::new(0.0..=1.0, progress);

        let enrollment_content = Column::new()
            .push(Text::new("Enregistrement de Visage").size(24))
            .push(preview_display)
            .push(
                Column::new()
                    .push(progress_bar)
                    .push(Text::new(format!("Progression: {}", progress_text)))
                    .spacing(5)
                    .width(Length::Fill)
                    .padding(20),
            )
            .push(
                Row::new()
                    .push(
                        Button::new(Text::new("▶ Démarrer"))
                            .on_press(Message::StartCapture)
                            .padding(10),
                    )
                    .push(
                        Button::new(Text::new("⏹ Arrêter"))
                            .on_press(Message::StopCapture)
                            .padding(10),
                    )
                    .push(
                        Button::new(Text::new("🏠 Accueil"))
                            .on_press(Message::GoToHome)
                            .padding(10),
                    )
                    .spacing(10)
                    .width(Length::Fill)
                    .padding(20),
            )
            .width(Length::Fill)
            .spacing(10)
            .padding(20);

        Container::new(enrollment_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        Row::new()
            .push(
                Container::new(Text::new("Paramètres"))
                    .width(Length::Fill)
                    .center_x(),
            )
            .into()
    }

    fn view_manage_faces(&self) -> Element<'_, Message> {
        Row::new()
            .push(
                Container::new(Text::new("Gérer les visages"))
                    .width(Length::Fill)
                    .center_x(),
            )
            .into()
    }
}
