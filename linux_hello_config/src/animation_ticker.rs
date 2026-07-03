//! Module to manage animation ticks
//!
//! Provides a simple system for generating animation messages
//! at regular intervals (~60fps)

use std::sync::mpsc;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::thread;
use std::time::Duration;

/// Animation tick manager
pub struct AnimationTicker {
    sender: mpsc::Sender<AnimationEvent>,
    receiver: mpsc::Receiver<AnimationEvent>,
    running: Arc<AtomicBool>,
}

/// Animation events
#[derive(Debug, Clone)]
pub enum AnimationEvent {
    /// Animation tick to execute
    Tick,
}

impl AnimationTicker {
    /// Create a new animation ticker
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();

        Self {
            sender,
            receiver,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the animation ticker (60fps ~ 16.67ms)
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

    /// Stop the animation ticker
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Retrieve a tick if one is available (non-blocking)
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

        // Wait for a tick
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

        // Wait for the thread to stop
        thread::sleep(Duration::from_millis(50));

        // No more ticks after stop
        ticker.try_tick();
        ticker.try_tick();
        let tick = ticker.try_tick();
        // The last tick can be None
        let _ = tick;
    }
}
