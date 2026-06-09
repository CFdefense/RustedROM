// Game Boy Joypad Input Controller Module
//
// This module implements the Game Boy's input system with button state tracking
// and proper register interface. Handles button matrix scanning and input processing
// with accurate timing and hardware behavior emulation.
//
// Hardware Interface:
// Register Address: 0xFF00 (Joypad Register)
// - Bit 5: Button Matrix Select (0 = select action buttons)
// - Bit 4: Direction Matrix Select (0 = select directional pad)
// - Bits 3-0: Button state output (0 = pressed, 1 = released)
//
// Matrix Scanning System:
// Button Matrix (when button_select = false):
//   - Bit 3: Start button state
//   - Bit 2: Select button state
//   - Bit 1: B button state
//   - Bit 0: A button state
//
// Direction Matrix (when direction_select = false):
//   - Bit 3: Down button state
//   - Bit 2: Up button state
//   - Bit 1: Left button state
//   - Bit 0: Right button state
//
// Input Processing:
// - Active low logic (0 = pressed, 1 = released)
// - Matrix selection determines which buttons are readable
// - Simultaneous matrix selection possible
// - Default state 0xCF when no buttons pressed
//
// Hardware Accuracy:
// - Accurate button matrix behavior matching original Game Boy
// - Proper active-low signal logic
// - Correct register bit mapping and selection
// - Matrix isolation preventing cross-talk between button groups

/// Represents the current state of all Game Boy controller buttons.
///
/// Stores the pressed/released state for all eight buttons using boolean values.
/// All buttons use active-high logic internally (true = pressed, false = released),
/// which is converted to active-low for hardware register output.
///
/// # Button Layout
///
/// Action Buttons:
/// - `start`: Start button (menu/pause)
/// - `select`: Select button (secondary menu)
/// - `a`: A button (primary action)
/// - `b`: B button (secondary action)
///
/// Directional Pad:
/// - `up`: D-Pad Up
/// - `down`: D-Pad Down
/// - `left`: D-Pad Left
/// - `right`: D-Pad Right
pub struct GamePadState {
    pub start: bool,
    pub select: bool,
    pub a: bool,
    pub b: bool,
    pub up: bool,
    pub down: bool,
    pub right: bool,
    pub left: bool,
}

impl GamePadState {
    pub fn new() -> Self {
        GamePadState {
            start: false,
            select: false,
            a: false,
            b: false,
            up: false,
            down: false,
            right: false,
            left: false,
        }
    }
}

/// Game Boy joypad controller with matrix selection and button state management.
///
/// Implements the Game Boy's button matrix scanning system where buttons are
/// organized into two groups (action buttons and directional pad) that can be
/// selectively read through the joypad register at 0xFF00.
///
/// # Hardware Behavior
///
/// The Game Boy uses a matrix scanning approach where:
/// - Bit 5 of 0xFF00 controls action button matrix selection
/// - Bit 4 of 0xFF00 controls directional pad matrix selection
/// - Setting a selection bit to 0 enables reading that button group
/// - Multiple groups can be selected simultaneously
pub struct GamePad {
    /// Button matrix selection flag (true = not selected, false = selected)
    pub button_select: bool,
    /// Direction matrix selection flag (true = not selected, false = selected)
    pub direction_select: bool,
    /// Current state of all controller buttons
    pub state: GamePadState,
}

impl GamePad {
    /// Creates a new GamePad with default settings.
    ///
    /// Initializes the gamepad with both matrix selections disabled (true)
    /// and all buttons in released state.
    ///
    /// # Returns
    ///
    /// A GamePad instance with default configuration.
    pub fn new() -> Self {
        GamePad {
            button_select: false,
            direction_select: false,
            state: GamePadState::new(),
        }
    }

    /// Returns the button matrix selection state.
    ///
    /// # Returns
    ///
    /// `true` if button matrix is not selected, `false` if selected (readable).
    pub fn gamepad_button_selection(&self) -> bool {
        self.button_select
    }

    /// Returns the direction matrix selection state.
    ///
    /// # Returns
    ///
    /// `true` if direction matrix is not selected, `false` if selected (readable).
    pub fn gamepad_direction_selection(&self) -> bool {
        self.direction_select
    }

    /// Sets the matrix selection from a joypad register write.
    ///
    /// Updates which button groups are readable based on the value written
    /// to the joypad register (0xFF00). Bit 5 controls action buttons,
    /// bit 4 controls directional pad.
    ///
    /// # Arguments
    ///
    /// * `value` - The byte value written to 0xFF00
    ///
    /// # Bit Mapping
    ///
    /// - Bit 5: Button matrix select (0 = select, 1 = not selected)
    /// - Bit 4: Direction matrix select (0 = select, 1 = not selected)
    pub fn gamepad_set_selection(&mut self, value: u8) {
        self.button_select = (value & 0x20) != 0;
        self.direction_select = (value & 0x10) != 0;
    }

    /// Generates the joypad register output value based on current button states.
    ///
    /// Converts the internal button state to the hardware register format with
    /// active-low logic. Only buttons in the selected matrix groups are reflected
    /// in the output. Returns 0xCF as the default value when no buttons are pressed.
    ///
    /// # Returns
    ///
    /// An 8-bit value representing the joypad register state:
    /// - Bits 7-6: Always 1 (unused)
    /// - Bit 5: Button select (reflects input)
    /// - Bit 4: Direction select (reflects input)
    /// - Bits 3-0: Button states (0 = pressed, 1 = released)
    ///
    /// # Button Matrix Output
    ///
    /// When button matrix is selected (button_select = false):
    /// - Bit 3: Start (0 = pressed)
    /// - Bit 2: Select (0 = pressed)
    /// - Bit 1: B (0 = pressed)
    /// - Bit 0: A (0 = pressed)
    ///
    /// When direction matrix is selected (direction_select = false):
    /// - Bit 3: Down (0 = pressed)
    /// - Bit 2: Up (0 = pressed)
    /// - Bit 1: Left (0 = pressed)
    /// - Bit 0: Right (0 = pressed)
    ///
    /// # Hardware Behavior
    ///
    /// Multiple matrix selections can be active simultaneously, allowing
    /// reading from both button groups at once (though this is uncommon).
    pub fn get_gamepad_output(&self) -> u8 {
        let mut output: u8 = 0xCF;

        if !self.gamepad_button_selection() {
            if self.state.start {
                output &= !(1 << 3);
            }
            if self.state.select {
                output &= !(1 << 2);
            }
            if self.state.b {
                output &= !(1 << 1);
            }
            if self.state.a {
                output &= !(1 << 0);
            }
        }

        if !self.gamepad_direction_selection() {
            if self.state.down {
                output &= !(1 << 3);
            }
            if self.state.up {
                output &= !(1 << 2);
            }
            if self.state.left {
                output &= !(1 << 1);
            }
            if self.state.right {
                output &= !(1 << 0);
            }
        }

        return output;
    }
}
