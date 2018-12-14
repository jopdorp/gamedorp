use std::cell::Cell;

use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::Sdl;

use ui::ButtonState;

pub struct Controller {
    buttons: Cell<::ui::Buttons>,
    #[allow(dead_code)]
    controller: Option<GameController>,
    x_axis_state: Cell<AxisState>,
    y_axis_state: Cell<AxisState>,
}

impl Controller {
    pub fn new(sdl2: &Sdl) -> Controller {
        // Attempt to add a game controller

        let game_controller_subsystem = sdl2.game_controller().unwrap();
        let njoysticks = match game_controller_subsystem.num_joysticks() {
            Ok(n) => {
                info!("found {} joysticks!\n", n);
                n
            },
            Err(e) => {
                error!("Can't enumarate joysticks: {:?}\n", e);
                0
            }
        };

        let mut controller = None;

        // For now we just take the first controller we manage to open
        // (if any)
        for id in 0..njoysticks {
            info!("Checking if joystick {} has game controller mapping\n", id);
            if game_controller_subsystem.is_game_controller(id) {
                match game_controller_subsystem.open(id) {
                    Ok(c) => {
                        // We managed to find and open a game controller,
                        // exit the loop
                        info!("Successfully opened \"{}\"\n", c.name());
                        controller = Some(c);
                        break;
                    }
                    Err(e) => info!("failed: {:?}\n", e),
                }
            }else{
                info!("Joystick {} has no mapping\n", id);
            }
        }

        match controller {
            Some(_) => print!("Controller support enabled"),
            None => print!("No controller found"),
        }

        Controller {
            buttons: Cell::new(::ui::Buttons::new(ButtonState::Up)),
            controller: controller,
            x_axis_state: Cell::new(AxisState::Neutral),
            y_axis_state: Cell::new(AxisState::Neutral),
        }
    }

    pub fn update(&self, sdl2: &Sdl) -> ::ui::Event {
        let mut event = ::ui::Event::None;

        let mut event_pump = match sdl2.event_pump() {
            Ok(d) => d,
            Err(e) => panic!("{}", e),
        };

        for e in event_pump.poll_iter() {
            match e {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => event = ::ui::Event::PowerOff,
                Event::KeyDown { keycode: key, .. } => {
                    if let Some(key) = key {
                        self.update_key(key, ButtonState::Down)
                    }
                }
                Event::KeyUp { keycode: key, .. } => {
                    if let Some(key) = key {
                        self.update_key(key, ButtonState::Up)
                    }
                }
                Event::ControllerButtonDown { button, .. } => {
                    self.update_button(button, ButtonState::Down)
                }
                Event::ControllerButtonUp { button, .. } => {
                    self.update_button(button, ButtonState::Up)
                }
                Event::ControllerAxisMotion {
                    axis, value: val, ..
                } => self.update_axis(axis, val),
                Event::Quit { .. } => event = ::ui::Event::PowerOff,
                _ => (),
            }
        }

        event
    }

    pub fn buttons(&self) -> &Cell<::ui::Buttons> {
        &self.buttons
    }

    /// Update key state. For now keybindings are hardcoded.
    fn update_key(&self, key: Keycode, state: ButtonState) {
        let mut b = self.buttons.get();

        match key {
            Keycode::Up => b.up = state,
            Keycode::Down => b.down = state,
            Keycode::Left => b.left = state,
            Keycode::Right => b.right = state,
            Keycode::LAlt => b.a = state,
            Keycode::LCtrl => b.b = state,
            Keycode::Return => b.start = state,
            Keycode::RShift => b.select = state,
            _ => (),
        }

        self.buttons.set(b);
    }

    /// Same as update_key but for controller buttons
    fn update_button(&self, button: Button, state: ButtonState) {
        let mut b = self.buttons.get();

        match button {
            //gameboy has a nd b swapped
            Button::B => b.a = state,
            Button::A => b.b = state,
            Button::DPadLeft => b.left = state,
            Button::DPadRight => b.right = state,
            Button::DPadUp => b.up = state,
            Button::DPadDown => b.down = state,
            Button::Start => b.start = state,
            Button::Back => b.select = state,
            _ => (),
        }

        self.buttons.set(b);
    }

    /// Map left stick X/Y to directional buttons
    fn update_axis(&self, axis: Axis, val: i16) {
        let mut b = self.buttons.get();

        let state = AxisState::from_value(val);

        match axis {
            Axis::LeftX => {
                if state != self.x_axis_state.get() {
                    self.x_axis_state.set(state);

                    b.left = state.down_if_negative();
                    b.right = state.down_if_positive();
                }
            }
            Axis::LeftY => {
                if state != self.y_axis_state.get() {
                    self.y_axis_state.set(state);

                    b.up = state.down_if_negative();
                    b.down = state.down_if_positive();
                }
            }
            _ => (),
        }

        self.buttons.set(b);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AxisState {
    Neutral,
    Negative,
    Positive,
}

impl AxisState {
    fn from_value(val: i16) -> AxisState {
        if val > AXIS_DEAD_ZONE {
            AxisState::Positive
        } else if val < -AXIS_DEAD_ZONE {
            AxisState::Negative
        } else {
            AxisState::Neutral
        }
    }

    fn down_if_negative(self) -> ButtonState {
        if self == AxisState::Negative {
            ButtonState::Down
        } else {
            ButtonState::Up
        }
    }

    fn down_if_positive(self) -> ButtonState {
        if self == AxisState::Positive {
            ButtonState::Down
        } else {
            ButtonState::Up
        }
    }
}

/// The controller axis moves in a range from -32768 to +32767. To
/// avoid spurious events this constant says how far from 0 the axis
/// has to move for us to register the event.
const AXIS_DEAD_ZONE: i16 = 10_000;
