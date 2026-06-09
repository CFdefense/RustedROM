// Game Boy CPU Register Management Module
//
// This module implements the Game Boy CPU's register set including 8-bit
// general purpose registers, 16-bit register pairs, and the specialized flags register.
// Provides efficient register access and flag manipulation for CPU operations.
//
// Register Architecture:
// 8-bit General Purpose Registers:
// - A: Accumulator - Primary register for arithmetic and logic operations
// - B, C: BC Register Pair - General purpose with special functions (C for ports)
// - D, E: DE Register Pair - General purpose with pointer capabilities
// - H, L: HL Register Pair - Primary pointer register for memory operations
//
// Special Registers:
// - F: Flags Register - Contains condition flags from arithmetic/logic operations
//
// Flag Bit Positions:
// - Bit 7: Zero Flag (Z)
// - Bit 6: Subtract Flag (N)
// - Bit 5: Half Carry Flag (H)
// - Bit 4: Carry Flag (C)
// - Bits 3-0: Unused (always 0)
//
// Register Pair Encoding:
// - High byte stored in left register, low byte in right register
// - AF: A (high), F (low) - Accumulator and flags
// - BC: B (high), C (low) - General purpose pair
// - DE: D (high), E (low) - General purpose pair
// - HL: H (high), L (low) - Memory pointer pair
//
// Performance Optimization:
// - Direct register access without indirection
// - Efficient bit manipulation for flag operations
// - Zero-copy register pair operations
// - Optimized flag register conversions
//
// Hardware Accuracy:
// - Exact flag behavior matching original Game Boy CPU
// - Proper bit positions for all flags
// - Accurate unused bit handling in flags register

// FLAG POSITIONS FOR FLAGS REGISTER
const ZERO_FLAG_BYTE_POSITION: u8 = 7;
const SUBTRACT_FLAG_BYTE_POSITION: u8 = 6;
const HALF_CARRY_FLAG_BYTE_POSITION: u8 = 5;
const CARRY_FLAG_BYTE_POSITION: u8 = 4;

/// Game Boy CPU register set.
///
/// Contains all 8-bit general purpose registers and the flags register.
/// Provides methods for accessing registers as 16-bit pairs (AF, BC, DE, HL).
#[derive(Debug)]
pub struct Registers {
    /// Accumulator register - primary 8-bit register for most operations
    pub a: u8,
    /// B register - high byte of BC register pair
    pub b: u8,
    /// C register - low byte of BC register pair (also used for I/O port addressing)
    pub c: u8,
    /// D register - high byte of DE register pair
    pub d: u8,
    /// E register - low byte of DE register pair
    pub e: u8,
    /// Flags register - contains condition flags from arithmetic/logic operations
    pub f: FlagsRegister,
    /// H register - high byte of HL register pair (memory addressing)
    pub h: u8,
    /// L register - low byte of HL register pair (memory addressing)
    pub l: u8,
}

/// Game Boy CPU flags register.
///
/// Contains four condition flags that are set/cleared by arithmetic and logic operations.
/// Stored as individual boolean fields for efficient access, with conversion methods
/// to/from byte representation for register operations.
///
/// # Flag Meanings
///
/// - `zero` (Z): Set when an arithmetic operation results in zero
/// - `subtract` (N): Set when the last operation was a subtraction
/// - `half_carry` (H): Set when carry/borrow occurs from bit 3 to bit 4
/// - `carry` (C): Set when carry/borrow occurs from bit 7
#[derive(Debug)]
pub struct FlagsRegister {
    /// Zero flag (Z) - bit 7 - set when arithmetic operation results in zero
    pub zero: bool,
    /// Subtract flag (N) - bit 6 - set when last operation was subtraction
    pub subtract: bool,
    /// Half carry flag (H) - bit 5 - set when carry from bit 3 to bit 4 occurs
    pub half_carry: bool,
    /// Carry flag (C) - bit 4 - set when carry from bit 7 or borrow occurs
    pub carry: bool,
}

impl Registers {
    /// Returns the AF register pair as a 16-bit value.
    ///
    /// # Returns
    ///
    /// 16-bit value with A in high byte and F in low byte.
    pub fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | u8::from(&self.f) as u16
    }

    /// Returns the BC register pair as a 16-bit value.
    ///
    /// # Returns
    ///
    /// 16-bit value with B in high byte and C in low byte.
    pub fn get_bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    /// Returns the DE register pair as a 16-bit value.
    ///
    /// # Returns
    ///
    /// 16-bit value with D in high byte and E in low byte.
    pub fn get_de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    /// Returns the HL register pair as a 16-bit value.
    ///
    /// # Returns
    ///
    /// 16-bit value with H in high byte and L in low byte.
    pub fn get_hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    /// Sets the AF register pair from a 16-bit value.
    ///
    /// # Arguments
    ///
    /// * `value` - 16-bit value where high byte goes to A, low byte to F
    pub fn set_af(&mut self, value: u16) {
        self.a = ((value & 0xFF00) >> 8) as u8;
        self.f = FlagsRegister::from((value & 0x00FF) as u8);
    }

    /// Sets the BC register pair from a 16-bit value.
    ///
    /// # Arguments
    ///
    /// * `value` - 16-bit value where high byte goes to B, low byte to C
    pub fn set_bc(&mut self, value: u16) {
        self.b = ((value & 0xFF00) >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }

    /// Sets the DE register pair from a 16-bit value.
    ///
    /// # Arguments
    ///
    /// * `value` - 16-bit value where high byte goes to D, low byte to E
    pub fn set_de(&mut self, value: u16) {
        self.d = ((value & 0xFF00) >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }

    /// Sets the HL register pair from a 16-bit value.
    ///
    /// # Arguments
    ///
    /// * `value` - 16-bit value where high byte goes to H, low byte to L
    pub fn set_hl(&mut self, value: u16) {
        self.h = ((value & 0xFF00) >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }
}

/// Converts a FlagsRegister reference to a byte representation.
///
/// Maps the four boolean flags to their corresponding bit positions in a byte.
/// Bits 3-0 are always 0 (unused).
impl std::convert::From<&FlagsRegister> for u8 {
    fn from(flag: &FlagsRegister) -> u8 {
        // Set Flag Bits In u8 Depending on Status in FlagsRegister
        (if flag.zero { 1 } else { 0 }) << ZERO_FLAG_BYTE_POSITION
            | (if flag.subtract { 1 } else { 0 }) << SUBTRACT_FLAG_BYTE_POSITION
            | (if flag.half_carry { 1 } else { 0 }) << HALF_CARRY_FLAG_BYTE_POSITION
            | (if flag.carry { 1 } else { 0 }) << CARRY_FLAG_BYTE_POSITION
    }
}

/// Converts a byte to a FlagsRegister.
///
/// Extracts the four flag bits from their respective positions and creates
/// a FlagsRegister with the corresponding boolean values. Bits 3-0 are ignored.
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
    /// Converts the flags register to its byte representation.
    ///
    /// Maps the four boolean flags to their corresponding bit positions.
    /// Bits 3-0 are always 0 (unused).
    ///
    /// # Returns
    ///
    /// An 8-bit value with flags in bits 7-4 and zeros in bits 3-0.
    ///
    /// # Bit Layout
    ///
    /// - Bit 7: Zero flag
    /// - Bit 6: Subtract flag
    /// - Bit 5: Half carry flag
    /// - Bit 4: Carry flag
    /// - Bits 3-0: Always 0
    pub fn as_byte(&self) -> u8 {
        (self.zero as u8) << 7
            | (self.subtract as u8) << 6
            | (self.half_carry as u8) << 5
            | (self.carry as u8) << 4
    }
}
