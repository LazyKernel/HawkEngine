use log::{trace, warn};
use winit::{event::{DeviceEvent, ElementState, KeyEvent, Modifiers, MouseButton}, keyboard::{KeyCode, ModifiersState, PhysicalKey}};


// Heavily inspired by winit_input_helper
#[derive(Clone)]
pub struct InputHelper {
    key_actions: Vec<ScanCodeAction>,
    mouse_actions: Vec<MouseAction>,
    keys_held: Vec<PhysicalKey>,
    mouse_buttons_held: Vec<MouseButton>,
    modifiers_state: ModifiersState,
    cursor_point: Option<(f32, f32)>,
    cursor_point_prev: Option<(f32, f32)>,
    mouse_diff_: (f32, f32),
}

impl Default for InputHelper {
    fn default() -> Self {
        Self::new()
    }
}


impl InputHelper {

    pub fn new() -> Self {
        Self { 
            key_actions: vec![],
            mouse_actions: vec![],
            keys_held: vec![],
            mouse_buttons_held: vec![],
            modifiers_state: ModifiersState::empty(),
            mouse_diff_: (0.0, 0.0),
            cursor_point: None,
            cursor_point_prev: None,
        }
    }

    // Utility functions

    pub fn held_shift(&self) -> bool {
        self.modifiers_state.shift_key()
    }

    pub fn held_control(&self) -> bool {
        self.modifiers_state.control_key()
    }

    pub fn key_held(&self, key: KeyCode) -> bool {
        let physical_key = PhysicalKey::Code(key);
        self.keys_held.contains(&physical_key)
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        let physical_key = PhysicalKey::Code(key);
        let searched_action = ScanCodeAction::Pressed(physical_key);
        self.key_actions.contains(&searched_action)
    }

    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        let searched_action = MouseAction::Pressed(button);
        self.mouse_actions.contains(&searched_action)
    }

    pub fn mouse_diff(&self) -> (f32, f32) {
        self.mouse_diff_
    }

    
    // Update functions

    pub fn step(&mut self) {
        self.key_actions.clear();
        self.mouse_actions.clear();
        self.mouse_diff_ = (0.0, 0.0);
        self.cursor_point_prev = self.cursor_point;
        // NOTE: modifiers state should manage itself
    }

    pub fn handle_keyboard_input(&mut self, event: KeyEvent) {
        match event.state {
            ElementState::Pressed => {
                if !self.keys_held.contains(&event.physical_key) {
                    self.key_actions.push(ScanCodeAction::Pressed(event.physical_key));
                    self.keys_held.push(event.physical_key);
                }
            },
            ElementState::Released => {
                self.key_actions.push(ScanCodeAction::Released(event.physical_key));
                self.keys_held.retain(|x| *x != event.physical_key);
            },
        }
    }

    pub fn handle_mouse_event(&mut self, state: ElementState, button: MouseButton) {
        match state {
            ElementState::Pressed => {
                if !self.mouse_buttons_held.contains(&button) {
                    self.mouse_actions.push(MouseAction::Pressed(button));
                    self.mouse_buttons_held.push(button);
                }
            },
            ElementState::Released => {
                self.mouse_actions.push(MouseAction::Released(button));
                self.mouse_buttons_held.retain(|x| *x != button);
            },
        }
    }

    pub fn handle_mouse_move_device(&mut self, device_event: DeviceEvent) {
        match device_event {
            DeviceEvent::MouseMotion { delta } => {
                // not sure if we can get multiple of these per frame
                self.mouse_diff_.0 += delta.0 as f32;
                self.mouse_diff_.1 += delta.1 as f32;
            },
            _ => trace!("Device event not implemented: {:?}", device_event),
        }
    }

    pub fn handle_touchpad_event(&mut self, _pressure: f32, stage: i64) {
        match stage {
            0 => {
                // 0 usually means released
                self.mouse_actions.push(MouseAction::Released(MouseButton::Left));
                self.mouse_buttons_held.retain(|x| *x != MouseButton::Left);
            },
            1 => {
                // 1 usually means pressed
                if !self.mouse_buttons_held.contains(&MouseButton::Left) {
                    self.mouse_actions.push(MouseAction::Pressed(MouseButton::Left));
                    self.mouse_buttons_held.push(MouseButton::Left);
                }
            },
            _ => warn!("Missing touchpad event stage {:?}", stage)
        };
    }

    pub fn handle_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers_state = modifiers.state();  
    }
}


#[derive(Clone, PartialEq)]
enum ScanCodeAction {
    Pressed(PhysicalKey),
    PressedOs(PhysicalKey),
    Released(PhysicalKey),
}

#[derive(Clone, PartialEq)]
enum MouseAction {
    Pressed(MouseButton),
    Released(MouseButton),
}
