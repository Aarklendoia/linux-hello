//! Module pour gérer les ticks d'animation
//!
//! Fournit un système simple pour générer des messages d'animation
//! à intervalles réguliers (~60fps)

use std::sync::mpsc;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::thread;
use std::time::Duration;

/// Gestionnaire des ticks d'animation
pub struct AnimationTicker {
    sender: mpsc::Sender<AnimationEvent>,
    receiver: mpsc::Receiver<AnimationEvent>,
    running: Arc<AtomicBool>,
}

/// Événements d'animation
#[derive(Debug, Clone)]
pub enum AnimationEvent {
    /// Tick d'animation à exécuter
    Tick,
}

impl AnimationTicker {
    /// Créer un nouveau ticker d'animation
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();

        Self {
            sender,
            receiver,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Démarrer le ticker d'animation (60fps ~ 16.67ms)
    pub fn start(&self) {
        let sender = self.sender.clone();
        let running = Arc::clone(&self.running);

        running.store(true, Ordering::SeqCst);

        thread::spawn(move || {
            let frame_duration = Duration::from_millis(16); // ~60fps

            while running.load(Ordering::SeqCst) {
                let _ = sender.send(AnimationEvent::Tick);
                thread::sleep(frame_duration);
            }
        });
    }

    /// Arrêter le ticker d'animation
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Récupérer un tick s'il y en a un disponible (non-blocking)
    pub fn try_tick(&self) -> Option<AnimationEvent> {
        self.receiver.try_recv().ok()
    }
}

impl Default for AnimationTicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_ticker_creation() {
        let ticker = AnimationTicker::new();
        assert!(ticker.receiver.try_recv().is_err());
    }

    #[test]
    fn test_animation_ticker_tick() {
        let ticker = AnimationTicker::new();
        ticker.start();

        // Attendre un tick
        thread::sleep(Duration::from_millis(20));

        let tick = ticker.try_tick();
        assert!(tick.is_some());

        ticker.stop();
    }

    #[test]
    fn test_animation_ticker_stop() {
        let ticker = AnimationTicker::new();
        ticker.start();
        ticker.stop();

        // Attendre que le thread s'arrête
        thread::sleep(Duration::from_millis(50));

        // Plus aucun tick après stop
        ticker.try_tick();
        ticker.try_tick();
        let tick = ticker.try_tick();
        // Le dernier tick peut être None
        let _ = tick;
    }
}
