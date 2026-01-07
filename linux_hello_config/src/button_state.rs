//! Button state tracking and animation

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonState {
    #[default]
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl ButtonState {
    /// Get the opacity modifier for this button state
    pub fn opacity(&self) -> f32 {
        match self {
            ButtonState::Normal => 1.0,
            ButtonState::Hover => 1.1,    // Slightly brighter
            ButtonState::Pressed => 0.9,  // Slightly darker
            ButtonState::Disabled => 0.5, // Much darker
        }
    }

    /// Get the scale factor for this button state
    pub fn scale(&self) -> f32 {
        match self {
            ButtonState::Normal => 1.0,
            ButtonState::Hover => 1.05,   // Slightly larger
            ButtonState::Pressed => 0.98, // Slightly smaller
            ButtonState::Disabled => 1.0,
        }
    }
}

/// Button state container for tracking button states
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonStates {
    pub start_capture_btn: ButtonState,
    pub stop_capture_btn: ButtonState,
    pub home_btn: ButtonState,
    pub enroll_btn: ButtonState,
    pub settings_btn: ButtonState,
    pub manage_btn: ButtonState,
}

impl ButtonStates {
    pub fn new() -> Self {
        Self {
            start_capture_btn: ButtonState::Normal,
            stop_capture_btn: ButtonState::Disabled,
            home_btn: ButtonState::Normal,
            enroll_btn: ButtonState::Normal,
            settings_btn: ButtonState::Normal,
            manage_btn: ButtonState::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_state_opacity() {
        assert_eq!(ButtonState::Normal.opacity(), 1.0);
        assert_eq!(ButtonState::Hover.opacity(), 1.1);
        assert_eq!(ButtonState::Pressed.opacity(), 0.9);
        assert_eq!(ButtonState::Disabled.opacity(), 0.5);
    }

    #[test]
    fn test_button_state_scale() {
        assert_eq!(ButtonState::Normal.scale(), 1.0);
        assert_eq!(ButtonState::Hover.scale(), 1.05);
        assert_eq!(ButtonState::Pressed.scale(), 0.98);
        assert_eq!(ButtonState::Disabled.scale(), 1.0);
    }

    #[test]
    fn test_button_states_default() {
        let states = ButtonStates::default();
        assert_eq!(states.start_capture_btn, ButtonState::Normal);
        assert_eq!(states.stop_capture_btn, ButtonState::Disabled);
        assert_eq!(states.home_btn, ButtonState::Normal);
    }

    #[test]
    fn test_button_states_new() {
        let states = ButtonStates::new();
        assert_eq!(states.start_capture_btn, ButtonState::Normal);
        assert_eq!(states.stop_capture_btn, ButtonState::Disabled);
    }
}
