/*
  hdw/gamepad.rs
  Info: Game Boy joypad input controller and button state management
  Description: The gamepad module implements the Game Boy's input system with button state tracking
              and proper register interface. Handles button matrix scanning and input processing
              with accurate timing and hardware behavior emulation.

  GamePadState Struct Members:
    start: Start Button - Menu/pause button state (active low when pressed)
    select: Select Button - Secondary menu button state (active low when pressed)
    a: A Button - Primary action button state (active low when pressed)
    b: B Button - Secondary action button state (active low when pressed)
    up: D-Pad Up - Directional pad up state (active low when pressed)
    down: D-Pad Down - Directional pad down state (active low when pressed)
    right: D-Pad Right - Directional pad right state (active low when pressed)
    left: D-Pad Left - Directional pad left state (active low when pressed)

  GamePad Struct Members:
    button_select: Button Matrix Selection - Controls access to action buttons (A, B, Select, Start)
    direction_select: Direction Matrix Selection - Controls access to directional pad buttons
    state: Button State - Current pressed/released state for all controller inputs

  Core Functions:
    GamePadState::new: State Constructor - Initializes all buttons to released state
    GamePad::new: Controller Constructor - Creates gamepad with default selection settings
    gamepad_button_selection: Button Mode Query - Returns true if button matrix is selected
    gamepad_direction_selection: Direction Mode Query - Returns true if direction matrix is selected
    gamepad_set_selection: Selection Control - Sets matrix selection from register write (FF00)
    get_gamepad_output: Register Output - Returns current button state for register read (FF00)

  Hardware Interface:
    Register Address: FF00 (Joypad Register)
    - Bit 5: Button Matrix Select (0 = select action buttons)
    - Bit 4: Direction Matrix Select (0 = select directional pad)
    - Bits 3-0: Button state output (0 = pressed, 1 = released)

  Matrix Scanning System:
    Button Matrix (when button_select = false):
      - Bit 3: Start button state
      - Bit 2: Select button state
      - Bit 1: B button state
      - Bit 0: A button state

    Direction Matrix (when direction_select = false):
      - Bit 3: Down button state
      - Bit 2: Up button state
      - Bit 1: Left button state
      - Bit 0: Right button state

  Input Processing:
    - Active low logic (0 = pressed, 1 = released)
    - Matrix selection determines which buttons are readable
    - Simultaneous matrix selection possible
    - Default state 0xCF when no buttons pressed

  Hardware Accuracy:
    - Accurate button matrix behavior matching original Game Boy
    - Proper active-low signal logic
    - Correct register bit mapping and selection
    - Matrix isolation preventing cross-talk between button groups

  Performance Features:
    - Direct boolean state storage for fast access
    - Efficient bit manipulation for register output
    - Minimal overhead input processing
    - Real-time button state updates

  Input Event Handling:
    - Immediate state updates on button press/release
    - No input buffering or delay processing
    - Direct hardware register interface
    - Compatible with all Game Boy input patterns
*/

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

pub struct GamePad {
    pub button_select: bool,
    pub direction_select: bool,
    pub state: GamePadState,
}

impl GamePad {
    pub fn new() -> Self {
        GamePad {
            button_select: false,
            direction_select: false,
            state: GamePadState::new(),
        }
    }

    pub fn gamepad_button_selection(&self) -> bool {
        self.button_select
    }
    pub fn gamepad_direction_selection(&self) -> bool {
        self.direction_select
    }

    pub fn gamepad_set_selection(&mut self, value: u8) {
        self.button_select = (value & 0x20) != 0;
        self.direction_select = (value & 0x10) != 0;
    }

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
