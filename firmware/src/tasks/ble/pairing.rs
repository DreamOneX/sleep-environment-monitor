#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BootButtonState {
    Pressed,
    #[default]
    Released,
}

impl BootButtonState {
    pub const fn from_active_low(is_low: bool) -> Self {
        if is_low {
            Self::Pressed
        } else {
            Self::Released
        }
    }

    pub const fn is_pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BlePairingState {
    #[default]
    Closed,
    Open {
        remaining_millis: u64,
    },
}

impl BlePairingState {
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open { .. })
    }

    pub const fn remaining_millis(self) -> u64 {
        match self {
            Self::Closed => 0,
            Self::Open { remaining_millis } => remaining_millis,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BlePairingEvent {
    None,
    WindowOpened,
    WindowExpired,
    AuthRecordsClearRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlePairingGesture {
    hold_millis: u64,
    clear_hold_millis: u64,
    window_millis: u64,
    pressed_millis: u64,
    opened_for_current_press: bool,
    cleared_for_current_press: bool,
    state: BlePairingState,
}

impl BlePairingGesture {
    pub const fn new(hold_millis: u64, clear_hold_millis: u64, window_millis: u64) -> Self {
        Self {
            hold_millis,
            clear_hold_millis,
            window_millis,
            pressed_millis: 0,
            opened_for_current_press: false,
            cleared_for_current_press: false,
            state: BlePairingState::Closed,
        }
    }

    pub const fn state(&self) -> BlePairingState {
        self.state
    }

    pub const fn pressed_millis(&self) -> u64 {
        self.pressed_millis
    }

    pub fn open_window(&mut self) -> BlePairingEvent {
        self.state = BlePairingState::Open {
            remaining_millis: self.window_millis,
        };
        self.opened_for_current_press = true;
        BlePairingEvent::WindowOpened
    }

    pub fn update(
        &mut self,
        button_state: BootButtonState,
        elapsed_millis: u64,
    ) -> BlePairingEvent {
        let expired = self.tick_pairing_window(elapsed_millis);

        if button_state.is_pressed() {
            self.pressed_millis = self.pressed_millis.saturating_add(elapsed_millis);
            if self.clear_hold_millis > 0
                && !self.cleared_for_current_press
                && self.pressed_millis >= self.clear_hold_millis
            {
                self.cleared_for_current_press = true;
                return BlePairingEvent::AuthRecordsClearRequested;
            }
            if !self.opened_for_current_press && self.pressed_millis >= self.hold_millis {
                return self.open_window();
            }
        } else {
            self.pressed_millis = 0;
            self.opened_for_current_press = false;
            self.cleared_for_current_press = false;
        }

        if expired {
            BlePairingEvent::WindowExpired
        } else {
            BlePairingEvent::None
        }
    }

    fn tick_pairing_window(&mut self, elapsed_millis: u64) -> bool {
        let BlePairingState::Open { remaining_millis } = self.state else {
            return false;
        };

        let remaining_millis = remaining_millis.saturating_sub(elapsed_millis);
        if remaining_millis == 0 {
            self.state = BlePairingState::Closed;
            true
        } else {
            self.state = BlePairingState::Open { remaining_millis };
            false
        }
    }
}
