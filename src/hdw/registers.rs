/*
  hdw/registers.rs
  Info: Game Boy CPU register management and flag operations
  Description: The registers module implements the Game Boy CPU's register set including 8-bit
              general purpose registers, 16-bit register pairs, and the specialized flags register.
              Provides efficient register access and flag manipulation for CPU operations.

  Register Architecture:
    8-bit General Purpose Registers:
    - A: Accumulator - Primary register for arithmetic and logic operations
    - B, C: BC Register Pair - General purpose with special functions (C for ports)
    - D, E: DE Register Pair - General purpose with pointer capabilities
    - H, L: HL Register Pair - Primary pointer register for memory operations

    Special Registers:
    - F: Flags Register - Contains condition flags from arithmetic/logic operations

  Registers Struct Members:
    a: Accumulator Register - Primary 8-bit register for most operations
    b: B Register - High byte of BC register pair
    c: C Register - Low byte of BC register pair (also used for I/O port addressing)
    d: D Register - High byte of DE register pair
    e: E Register - Low byte of DE register pair
    f: Flags Register - Special register containing condition flags
    h: H Register - High byte of HL register pair (memory addressing)
    l: L Register - Low byte of HL register pair (memory addressing)

  FlagsRegister Struct Members:
    zero: Zero Flag (Z) - Set when arithmetic operation results in zero
    subtract: Subtract Flag (N) - Set when last operation was subtraction
    half_carry: Half Carry Flag (H) - Set when carry from bit 3 to bit 4 occurs
    carry: Carry Flag (C) - Set when carry from bit 7 or borrow occurs

  Flag Bit Positions:
    - Bit 7: Zero Flag (Z)
    - Bit 6: Subtract Flag (N)
    - Bit 5: Half Carry Flag (H)
    - Bit 4: Carry Flag (C)
    - Bits 3-0: Unused (always 0)

  16-bit Register Pair Operations:
    get_af: AF Pair Reader - Returns A and F registers as 16-bit value
    get_bc: BC Pair Reader - Returns B and C registers as 16-bit value
    get_de: DE Pair Reader - Returns D and E registers as 16-bit value
    get_hl: HL Pair Reader - Returns H and L registers as 16-bit value
    set_af: AF Pair Writer - Sets A and F registers from 16-bit value
    set_bc: BC Pair Writer - Sets B and C registers from 16-bit value
    set_de: DE Pair Writer - Sets D and E registers from 16-bit value
    set_hl: HL Pair Writer - Sets H and L registers from 16-bit value

  Flag Conversion Operations:
    From<&FlagsRegister> for u8: Converts flag register to byte representation
    From<u8> for FlagsRegister: Converts byte to flag register structure
    as_byte: Direct flag register to byte conversion method

  Register Pair Encoding:
    - High byte stored in left register, low byte in right register
    - AF: A (high), F (low) - Accumulator and flags
    - BC: B (high), C (low) - General purpose pair
    - DE: D (high), E (low) - General purpose pair
    - HL: H (high), L (low) - Memory pointer pair

  Performance Optimization:
    - Direct register access without indirection
    - Efficient bit manipulation for flag operations
    - Zero-copy register pair operations
    - Optimized flag register conversions

  Hardware Accuracy:
    - Exact flag behavior matching original Game Boy CPU
    - Proper bit positions for all flags
    - Accurate unused bit handling in flags register
    - Register pair operations match hardware timing
*/

// FLAG POSITIONS FOR FLAGS REGISTER
const ZERO_FLAG_BYTE_POSITION: u8 = 7;
const SUBTRACT_FLAG_BYTE_POSITION: u8 = 6;
const HALF_CARRY_FLAG_BYTE_POSITION: u8 = 5;
const CARRY_FLAG_BYTE_POSITION: u8 = 4;

// Registers For Holding and Manipulating Data
#[derive(Debug)]
pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: FlagsRegister,
    pub h: u8,
    pub l: u8,
}

// Special Flags Register to act as u8 but be called as struct
#[derive(Debug)]
pub struct FlagsRegister {
    pub zero: bool,
    pub subtract: bool,
    pub half_carry: bool,
    pub carry: bool,
}

impl Registers {
    // Get Virtual 16-Bit Register -> Rust Returns Last Expression
    pub fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | u8::from(&self.f) as u16
    }
    pub fn get_bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    pub fn get_de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }
    pub fn get_hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    // Set Virtual 16-Bit Register mask bytes and shift
    pub fn set_af(&mut self, value: u16) {
        self.a = ((value & 0xFF00) >> 8) as u8;
        self.f = FlagsRegister::from((value & 0x00FF) as u8);
    }
    pub fn set_bc(&mut self, value: u16) {
        self.b = ((value & 0xFF00) >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }
    pub fn set_de(&mut self, value: u16) {
        self.d = ((value & 0xFF00) >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }
    pub fn set_hl(&mut self, value: u16) {
        self.h = ((value & 0xFF00) >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }
}

// Method to Convert Flag Register Struct to u8
impl std::convert::From<&FlagsRegister> for u8 {
    fn from(flag: &FlagsRegister) -> u8 {
        // Set Flag Bits In u8 Depending on Status in FlagsRegister
        (if flag.zero { 1 } else { 0 }) << ZERO_FLAG_BYTE_POSITION
            | (if flag.subtract { 1 } else { 0 }) << SUBTRACT_FLAG_BYTE_POSITION
            | (if flag.half_carry { 1 } else { 0 }) << HALF_CARRY_FLAG_BYTE_POSITION
            | (if flag.carry { 1 } else { 0 }) << CARRY_FLAG_BYTE_POSITION
    }
}

// Method to Convert u8 to Flag Register Struct
impl std::convert::From<u8> for FlagsRegister {
    fn from(byte: u8) -> Self {
        // Get Register Bitwise Values
        let zero = ((byte >> ZERO_FLAG_BYTE_POSITION) & 0xb1) != 0;
        let subtract = ((byte >> SUBTRACT_FLAG_BYTE_POSITION) & 0xb1) != 0;
        let half_carry = ((byte >> HALF_CARRY_FLAG_BYTE_POSITION) & 0xb1) != 0;
        let carry = ((byte >> CARRY_FLAG_BYTE_POSITION) & 0xb1) != 0;

        // Remake Register
        FlagsRegister {
            zero,
            subtract,
            half_carry,
            carry,
        }
    }
}

impl FlagsRegister {
    pub fn as_byte(&self) -> u8 {
        (self.zero as u8) << 7
            | (self.subtract as u8) << 6
            | (self.half_carry as u8) << 5
            | (self.carry as u8) << 4
    }
}
