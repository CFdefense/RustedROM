/**
 * CPU Utilities Module - Helper Functions for CPU Operations
 *
 * This module provides essential utility functions that support CPU instruction
 * execution, including register access, flag management, conditional testing,
 * and program flow control. These utilities are used throughout the CPU
 * implementation to maintain consistent behavior and reduce code duplication.
 *
 * Core Functionality:
 *
 * Register Access:
 * - match_hl(): Maps HLTarget enums to actual register values
 * - match_n16(): Maps register pair enums to 16-bit values
 *
 * Conditional Testing:
 * - match_jump(): Evaluates jump conditions based on CPU flags
 * - Supports Z/NZ (zero), C/NC (carry) condition codes
 *
 * Flag Management:
 * Specialized flag update functions for different instruction types:
 * - Arithmetic: set_flags_after_add_a(), set_flags_after_sub()
 * - Logical: set_flags_after_and(), set_flags_after_xor_or()
 * - Bit operations: set_flags_after_bit(), set_flags_after_pref_op()
 * - Special: set_flags_after_daa(), set_flags_after_cpl()
 *
 * Program Flow:
 * - goto_addr(): Handles jumps, calls, and returns with optional stack push
 * - Manages conditional execution and PC updates
 *
 * Debug Support:
 * - print_step_info(): Outputs detailed CPU state information
 * - log_cpu_state(): File-based logging for debugging
 * - Conditional logging based on debug mode settings
 *
 * Flag Update Algorithms:
 * Each flag setting function implements the precise Game Boy flag update rules:
 * - Zero flag: Set when result equals zero
 * - Subtract flag: Set for subtraction operations, cleared for addition
 * - Half carry: Set when carry/borrow occurs between bits 3 and 4
 * - Carry flag: Set when carry/borrow occurs from most significant bit
 *
 * The utilities ensure consistent and accurate CPU behavior across all
 * instruction implementations while providing debugging capabilities.
 */
/*

    Helper File to Contain Helper Utilization Functions For CPU Execute Operations

*/
use super::emu::emu_cycles;
use super::stack::stack_push16;
use crate::hdw::cpu::CPU;
use crate::hdw::emu::EmuContext;
use crate::hdw::instructions::*;
use core::panic;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Resolves a 16-bit register pair target to its actual value.
///
/// Maps AddN16Target enum variants to their corresponding 16-bit register
/// pair values or stack pointer. Used by 16-bit arithmetic and load operations.
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `target` - The 16-bit register pair to resolve (BC, DE, HL, or SP)
///
/// # Returns
///
/// The 16-bit value of the specified register pair or stack pointer.
pub fn match_n16(cpu: &mut CPU, target: AddN16Target) -> u16 {
    let reg_target = match target {
        AddN16Target::BC => cpu.registers.get_bc(),
        AddN16Target::DE => cpu.registers.get_de(),
        AddN16Target::HL => cpu.registers.get_hl(),
        AddN16Target::SP => cpu.sp,
    };
    reg_target
}

// Method to match a Jump Condition
pub fn match_jump(cpu: &mut CPU, test: &JumpTest) -> bool {
    let jump_condition: bool = match test {
        JumpTest::NotZero => !cpu.registers.f.zero,
        JumpTest::NotCarry => !cpu.registers.f.carry,
        JumpTest::Zero => cpu.registers.f.zero,
        JumpTest::Carry => cpu.registers.f.carry,
        JumpTest::Always => true,
        JumpTest::HL => panic!("HL BAD"),
    };
    jump_condition
}

// Method to match a HL Target
pub fn match_hl(cpu: &mut CPU, target: &HLTarget) -> u8 {
    let reg_target = match target {
        HLTarget::A => cpu.registers.a,
        HLTarget::B => cpu.registers.b,
        HLTarget::C => cpu.registers.c,
        HLTarget::D => cpu.registers.d,
        HLTarget::E => cpu.registers.e,
        HLTarget::H => cpu.registers.h,
        HLTarget::L => cpu.registers.l,
        HLTarget::HL => cpu.bus.read_byte(None, cpu.registers.get_hl()),
    };
    reg_target
}

/// Updates CPU flags after an INC (increment) operation.
///
/// Sets flags according to Game Boy INC instruction behavior: Z flag based on result,
/// N flag cleared (addition), H flag set if carry from bit 3 to 4, C flag unchanged.
///
/// # Opcodes
///
/// 0x04, 0x14, 0x24, 0x34, 0x0C, 0x1C, 0x2C, 0x3C
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the increment operation
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Set if carry from bit 3 to bit 4
/// * C: Unchanged
pub fn set_flags_after_inc(cpu: &mut CPU, result: u8) {
    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry = (result & 0x0F) == 0;
}

/// Updates CPU flags after a DEC (decrement) operation.
///
/// Sets flags according to Game Boy DEC instruction behavior: Z flag based on result,
/// N flag set (subtraction), H flag set if borrow from bit 4 to 3, C flag unchanged.
///
/// # Opcodes
///
/// 0x05, 0x15, 0x25, 0x35, 0x0D, 0x1D, 0x2D, 0x3D
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the decrement operation
/// * `original_value` - The value before decrement
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Set (1)
/// * H: Set if borrow from bit 4 to bit 3
/// * C: Unchanged
pub fn set_flags_after_dec(cpu: &mut CPU, result: u8, original_value: u8) {
    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = true;
    cpu.registers.f.half_carry = (original_value & 0x0F) == 0x00;
}

/// Updates CPU flags after an ADC (add with carry) operation.
///
/// Sets flags according to Game Boy ADC instruction behavior, including the
/// carry flag from the previous operation in the calculation.
///
/// # Opcodes
///
/// 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0xCE
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the ADC operation
/// * `original_value` - The original accumulator value
/// * `immediate_operand` - The value being added
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Set if carry from bit 3 to bit 4
/// * C: Set if carry from bit 7
pub fn set_flags_after_adc(cpu: &mut CPU, result: u8, original_value: u8, immediate_operand: u8) {
    let carry_in = cpu.registers.f.carry as u8;

    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry =
        ((original_value & 0x0F) + (immediate_operand & 0x0F) + carry_in) > 0x0F;
    cpu.registers.f.carry =
        ((original_value as u16) + (immediate_operand as u16) + (carry_in as u16)) > 0xFF;
}

/// Updates CPU flags after a SUB or SBC (subtract) operation.
///
/// Sets flags according to Game Boy SUB instruction behavior: Z flag based on result,
/// N flag set (subtraction), H flag set if borrow from bit 4, C flag set if borrow occurred.
///
/// # Opcodes
///
/// 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0xD6, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xDE
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the subtraction
/// * `original_value` - The original accumulator value
/// * `immediate_operand` - The value being subtracted
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Set (1)
/// * H: Set if borrow from bit 4 to bit 3
/// * C: Set if borrow occurred (original < operand)
pub fn set_flags_after_sub(cpu: &mut CPU, result: u8, original_value: u8, immediate_operand: u8) {
    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = true;
    cpu.registers.f.half_carry = (original_value & 0xF) < (immediate_operand & 0xF);
    cpu.registers.f.carry = original_value < immediate_operand;
}

/// Updates CPU flags after an AND (bitwise AND) operation.
///
/// Sets flags according to Game Boy AND instruction behavior: Z flag based on result,
/// N and C flags cleared, H flag always set.
///
/// # Opcodes
///
/// 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xE6
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the AND operation
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Set (1)
/// * C: Reset (0)
pub fn set_flags_after_and(cpu: &mut CPU, result: u8) {
    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry = true;
    cpu.registers.f.carry = false;
}

/// Updates CPU flags after XOR or OR (bitwise logical) operations.
///
/// Sets flags according to Game Boy XOR/OR instruction behavior: Z flag based on result,
/// all other flags cleared.
///
/// # Opcodes
///
/// XOR: 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xEE
/// OR: 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xF6
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the XOR or OR operation
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Reset (0)
/// * C: Reset (0)
pub fn set_flags_after_xor_or(cpu: &mut CPU, result: u8) {
    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry = false;
    cpu.registers.f.carry = false;
}

/// Updates CPU flags after a CP (compare) operation.
///
/// Sets flags as if a SUB operation was performed, but without storing the result.
/// Used to compare the accumulator with another value.
///
/// # Opcodes
///
/// 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xFE
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `a` - The accumulator value
/// * `b` - The value to compare against
///
/// # Flag Updates
///
/// * Z: Set if a equals b
/// * N: Set (1)
/// * H: Set if borrow from bit 4
/// * C: Set if a < b
pub fn set_flags_after_cp(cpu: &mut CPU, a: u8, b: u8) {
    cpu.registers.f.zero = a == b;
    cpu.registers.f.subtract = true;
    cpu.registers.f.half_carry = (a & 0x0F) < (b & 0x0F);
    cpu.registers.f.carry = a < b;
}

/// Updates CPU flags after a BIT (bit test) operation.
///
/// Tests a specific bit in a register or memory location. Sets Z flag if the
/// tested bit is 0, always sets H flag, always clears N flag, leaves C flag unchanged.
///
/// # Opcodes
///
/// CB 0x40-0x7F (64 opcodes for testing bits 0-7 in all 8 registers/[HL])
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `bit` - The bit mask to test (e.g., 0x01 for bit 0, 0x80 for bit 7)
/// * `target_register` - The register or memory value being tested
///
/// # Flag Updates
///
/// * Z: Set if the tested bit is 0
/// * N: Reset (0)
/// * H: Set (1)
/// * C: Unchanged
pub fn set_flags_after_bit(cpu: &mut CPU, bit: u8, target_register: u8) {
    cpu.registers.f.zero = (target_register & bit) == 0;
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry = true;
}

/// Updates CPU flags after CB-prefixed rotate/shift operations.
///
/// Used by RLC, RRC, RL, RR, SLA, SRA, and SRL instructions. Sets Z flag based
/// on result, C flag based on the bit shifted out, clears N and H flags.
///
/// # Opcodes
///
/// CB 0x00-0x2F (48 opcodes for various rotate/shift operations)
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `bit` - The bit that was shifted out (0 or non-zero)
/// * `reg_target` - The result after the rotate/shift operation
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Reset (0)
/// * C: Set if bit shifted out is non-zero
pub fn set_flags_after_pref_op(cpu: &mut CPU, bit: u8, reg_target: u8) {
    cpu.registers.f.zero = reg_target == 0;
    cpu.registers.f.carry = bit != 0;
    cpu.registers.f.half_carry = false;
    cpu.registers.f.subtract = false;
}

/// Updates CPU flags after a CPL (complement) operation.
///
/// Inverts all bits in the accumulator. Sets N and H flags, leaves Z and C unchanged.
///
/// # Opcodes
///
/// 0x2F
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
///
/// # Flag Updates
///
/// * Z: Unchanged
/// * N: Set (1)
/// * H: Set (1)
/// * C: Unchanged
pub fn set_flags_after_cpl(cpu: &mut CPU) {
    cpu.registers.f.subtract = true;
    cpu.registers.f.half_carry = true;
}

/// Updates CPU flags after a SWAP (swap nibbles) operation.
///
/// Swaps the upper and lower nibbles of a register or memory location.
/// Sets Z flag based on result, clears all other flags.
///
/// # Opcodes
///
/// CB 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `reg_target` - The result after swapping nibbles
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Reset (0)
/// * C: Reset (0)
pub fn set_flags_after_swap(cpu: &mut CPU, reg_target: u8) {
    cpu.registers.f.zero = reg_target == 0;
    cpu.registers.f.carry = false;
    cpu.registers.f.half_carry = false;
    cpu.registers.f.subtract = false;
}

/// Updates CPU flags after a DAA (decimal adjust accumulator) operation.
///
/// Adjusts the accumulator to represent a valid BCD (Binary Coded Decimal) value
/// after an addition or subtraction. Clears H flag, sets C flag if carry occurred,
/// sets Z flag if result is zero.
///
/// # Opcodes
///
/// 0x27
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `carry` - Whether a carry occurred during the BCD adjustment
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Unchanged
/// * H: Reset (0)
/// * C: Set based on carry parameter
pub fn set_flags_after_daa(cpu: &mut CPU, carry: bool) {
    cpu.registers.f.half_carry = false;
    cpu.registers.f.carry = carry;
    cpu.registers.f.zero = cpu.registers.a == 0;
}

/// Updates CPU flags after non-prefixed rotate operations (RRA, RLA, RRCA, RLCA).
///
/// These accumulator-only rotates clear the Z flag (unlike their CB-prefixed counterparts),
/// clear N and H flags, and set C flag based on the bit rotated out.
///
/// # Opcodes
///
/// 0x07 (RLCA), 0x17 (RLA), 0x0F (RRCA), 0x1F (RRA)
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `bit` - The bit that was rotated out (0 or non-zero)
///
/// # Flag Updates
///
/// * Z: Reset (0) - differs from CB-prefixed versions
/// * N: Reset (0)
/// * H: Reset (0)
/// * C: Set if bit rotated out is non-zero
pub fn set_flags_after_no_pre_rl_rr(cpu: &mut CPU, bit: u8) {
    cpu.registers.f.zero = false;
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry = false;
    cpu.registers.f.carry = bit != 0;
}

/// Updates CPU flags after an ADD A (8-bit addition to accumulator) operation.
///
/// Sets flags according to Game Boy ADD instruction behavior. Handles both
/// register-to-register and immediate value additions.
///
/// # Opcodes
///
/// 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0xC6
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `reg_target` - The value being added to the accumulator
/// * `original` - The original accumulator value before addition
/// * `is_d8` - Whether this is an immediate value (d8) addition
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Reset (0)
/// * H: Set if carry from bit 3 to bit 4
/// * C: Set if carry from bit 7
pub fn set_flags_after_add_a(cpu: &mut CPU, reg_target: u8, original: u8, is_d8: bool) {
    if is_d8 {
        cpu.registers.f.zero = cpu.registers.a == 0;
        cpu.registers.f.subtract = false;
        cpu.registers.f.half_carry = ((original & 0x0F) + (reg_target & 0x0F)) > 0x0F;
        cpu.registers.f.carry = (cpu.registers.a < original) || (cpu.registers.a < reg_target);
    } else {
        cpu.registers.f.zero = cpu.registers.a == 0;
        cpu.registers.f.subtract = false;
        cpu.registers.f.half_carry = (original & 0x0F) + (reg_target & 0x0F) > 0x0F;
        cpu.registers.f.carry = cpu.registers.a < original;
    }
}

/// Updates CPU flags after an ADD HL (16-bit addition to HL) operation.
///
/// Sets flags according to Game Boy 16-bit ADD instruction behavior. Z flag
/// is not affected, N flag is cleared, H and C flags set based on carries.
///
/// # Opcodes
///
/// 0x09, 0x19, 0x29, 0x39
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `operand1` - The first 16-bit operand (typically HL)
/// * `operand2` - The second 16-bit operand (BC, DE, HL, or SP)
///
/// # Flag Updates
///
/// * Z: Unchanged
/// * N: Reset (0)
/// * H: Set if carry from bit 11 to bit 12
/// * C: Set if carry from bit 15
pub fn set_flags_after_add_n16(cpu: &mut CPU, operand1: u16, operand2: u16) {
    cpu.registers.f.subtract = false;
    cpu.registers.f.half_carry = ((operand1 & 0x0FFF) + (operand2 & 0x0FFF)) > 0x0FFF;
    cpu.registers.f.carry = ((operand1 as u32) + (operand2 as u32)) > 0xFFFF;
}

/// Updates CPU flags after an LD HL, SP+e8 operation.
///
/// Loads SP plus a signed 8-bit offset into HL. Clears Z and N flags,
/// sets H and C flags based on carries from the low byte addition.
///
/// # Opcodes
///
/// 0xF8
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `original_sp` - The original stack pointer value
/// * `r8_signed` - The signed 8-bit offset
///
/// # Flag Updates
///
/// * Z: Reset (0)
/// * N: Reset (0)
/// * H: Set if carry from bit 3 to bit 4 of low byte
/// * C: Set if carry from bit 7 to bit 8 of low byte
pub fn set_flags_after_ld_spe8(cpu: &mut CPU, original_sp: u16, r8_signed: i8) {
    cpu.registers.f.zero = false;
    cpu.registers.f.subtract = false;

    let r8_unsigned = r8_signed as u8;
    let sp_low_byte = original_sp as u8;

    cpu.registers.f.half_carry = ((sp_low_byte & 0x0F) + (r8_unsigned & 0x0F)) > 0x0F;
    cpu.registers.f.carry = (sp_low_byte as u16 + r8_unsigned as u16) > 0xFF;
}

/// Updates CPU flags after an SBC (subtract with carry) operation.
///
/// Sets flags according to Game Boy SBC instruction behavior, including the
/// carry flag from the previous operation in the calculation.
///
/// # Opcodes
///
/// 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xDE
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `result` - The result of the SBC operation
/// * `original_a` - The original accumulator value
/// * `operand` - The value being subtracted
/// * `carry_in` - The carry flag value (0 or 1)
///
/// # Flag Updates
///
/// * Z: Set if result is zero
/// * N: Set (1)
/// * H: Set if borrow from bit 4
/// * C: Set if borrow occurred
pub fn set_flags_after_sbc(cpu: &mut CPU, result: u8, original_a: u8, operand: u8, carry_in: u8) {
    cpu.registers.f.zero = result == 0;
    cpu.registers.f.subtract = true;
    cpu.registers.f.half_carry =
        ((original_a & 0x0F) as i16 - (operand & 0x0F) as i16 - carry_in as i16) < 0;
    cpu.registers.f.carry = ((original_a as i16) - (operand as i16) - carry_in as i16) < 0;
}

/// Updates CPU flags after an ADD SP, r8 operation.
///
/// Adds a signed 8-bit value to the stack pointer. Clears Z and N flags,
/// sets H and C flags based on carries from the low byte addition.
///
/// # Opcodes
///
/// 0xE8
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `original_sp` - The original stack pointer value
/// * `r8_signed` - The signed 8-bit value to add
///
/// # Flag Updates
///
/// * Z: Reset (0)
/// * N: Reset (0)
/// * H: Set if carry from bit 3 to bit 4 of low byte
/// * C: Set if carry from bit 7 to bit 8 of low byte
pub fn set_flags_after_add_sp_r8(cpu: &mut CPU, original_sp: u16, r8_signed: i8) {
    cpu.registers.f.zero = false;
    cpu.registers.f.subtract = false;

    let r8_unsigned = r8_signed as u8;
    let sp_low_byte = original_sp as u8;

    cpu.registers.f.half_carry = ((sp_low_byte & 0x0F) + (r8_unsigned & 0x0F)) > 0x0F;
    cpu.registers.f.carry = (sp_low_byte as u16 + r8_unsigned as u16) > 0xFF;
}

/// Performs a conditional or unconditional jump to a target address.
///
/// Evaluates the jump condition and, if met, updates the program counter to the
/// target address. Optionally pushes the current PC to the stack (for CALL instructions).
/// Adds an extra cycle if the jump is taken.
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `address` - The target address to jump to
/// * `jump_test` - The condition to test (Always, Zero, NotZero, Carry, NotCarry)
/// * `push_pc` - Whether to push the current PC to the stack before jumping
///
/// # Returns
///
/// The updated program counter value.
pub fn goto_addr(cpu: &mut CPU, address: u16, jump_test: JumpTest, push_pc: bool) -> u16 {
    let jump = match_jump(cpu, &jump_test);

    if jump {
        if push_pc {
            stack_push16(cpu, cpu.pc, true);
        }
        cpu.pc = address;
        emu_cycles(cpu, 1);
    }
    cpu.pc
}

/// Prints the current CPU execution step to stdout.
///
/// Outputs detailed CPU state information including program counter, current instruction,
/// opcode bytes, register values, and flags. Used for real-time debugging and tracing.
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `ctx` - Shared emulator context containing tick count
/// * `log_ticks` - Whether to include tick count in the output
///
/// # Output Format
///
/// With ticks: TTTTTTTT - AAAA: INSTRUCTION (OP B1 B2) A: XX F: ZNHC BC: XXXX DE: XXXX HL: XXXX
///
/// Without ticks: AAAA: INSTRUCTION (OP B1 B2) A: XX F: ZNHC BC: XXXX DE: XXXX HL: XXXX
///
/// Where TTTTTTTT is tick count, AAAA is program counter, OP B1 B2 are opcode bytes,
/// and ZNHC are flag states (letter if set, dash if clear).
pub fn print_step_info(cpu: &mut CPU, ctx: &Arc<Mutex<EmuContext>>, log_ticks: bool) {
    let ticks = ctx.lock().unwrap().ticks;

    let instruction_name_display =
        cpu.curr_instruction
            .as_ref()
            .map_or("None".to_string(), |instr| {
                format!("{:?}", instr)
                    .split('(')
                    .next()
                    .unwrap_or("Unknown")
                    .to_string()
            });

    if log_ticks {
        print!(
            "\n{:08X} - {:04X}: {}\t({:02X} {:02X} {:02X}) A: {:02X} F: {}{}{}{} BC: {:04X} DE: {:04X} HL: {:04X}",
            ticks,
            cpu.pc,
            instruction_name_display,
            cpu.curr_opcode,
            cpu.bus.read_byte(None, cpu.pc.wrapping_add(1)),
            cpu.bus.read_byte(None, cpu.pc.wrapping_add(2)),
            cpu.registers.a,
            if cpu.registers.f.zero { 'Z' } else { '-' },
            if cpu.registers.f.subtract { 'N' } else { '-' },
            if cpu.registers.f.half_carry { 'H' } else { '-' },
            if cpu.registers.f.carry { 'C' } else { '-' },
            cpu.registers.get_bc(),
            cpu.registers.get_de(),
            cpu.registers.get_hl()
        );
    } else {
        print!(
            "\n{:04X}: {}\t({:02X} {:02X} {:02X}) A: {:02X} F: {}{}{}{} BC: {:04X} DE: {:04X} HL: {:04X}",
            cpu.pc,
            instruction_name_display,
            cpu.curr_opcode,
            cpu.bus.read_byte(None, cpu.pc.wrapping_add(1)),
            cpu.bus.read_byte(None, cpu.pc.wrapping_add(2)),
            cpu.registers.a,
            if cpu.registers.f.zero { 'Z' } else { '-' },
            if cpu.registers.f.subtract { 'N' } else { '-' },
            if cpu.registers.f.half_carry { 'H' } else { '-' },
            if cpu.registers.f.carry { 'C' } else { '-' },
            cpu.registers.get_bc(),
            cpu.registers.get_de(),
            cpu.registers.get_hl()
        );
    }
    let _ = std::io::stdout().flush();
}

/// Logs the current CPU state to a file for debugging.
///
/// Writes detailed CPU state information to logs/cpu_log.txt. Creates the logs
/// directory if it doesn't exist. Supports two output formats: one with tick count
/// and instruction details, another with full register dump and memory preview.
///
/// # Arguments
///
/// * `cpu` - Mutable CPU reference
/// * `ctx` - Shared emulator context containing tick count
/// * `log_ticks` - Whether to use the tick-based format (true) or register dump format (false)
///
/// # Output Formats
///
/// Tick format: TTTTTTTT - AAAA: INSTRUCTION (OP B1 B2) A:XX F:ZNHC BC:XXXX DE:XXXX HL:XXXX IE:XX IF:XX
///
/// Register format: A:XX F:XX B:XX C:XX D:XX E:XX H:XX L:XX SP:XXXX PC:XXXX PCMEM:XX,XX,XX,XX
///
/// # File Location
///
/// Logs are written to logs/cpu_log.txt in append mode.
pub fn log_cpu_state(cpu: &mut CPU, ctx: &Arc<Mutex<EmuContext>>, log_ticks: bool) {
    let ticks = ctx.lock().unwrap().ticks;
    let pcmem1 = cpu.bus.read_byte(None, cpu.pc.wrapping_add(1));
    let pcmem2 = cpu.bus.read_byte(None, cpu.pc.wrapping_add(2));

    let log_entry = if log_ticks {
        let instruction_name_display =
            cpu.curr_instruction
                .as_ref()
                .map_or("None".to_string(), |instr| {
                    format!("{:?}", instr)
                        .split('(')
                        .next()
                        .unwrap_or("Unknown")
                        .to_string()
                });
        format!(
            "{:08X} - {:04X}: {:<12}\t({:02X} {:02X} {:02X}) A:{:02X} F:{}{}{}{} BC:{:04X} DE:{:04X} HL:{:04X} IE:{:02X} IF:{:02X}",
            ticks,
            cpu.pc,
            instruction_name_display,
            cpu.curr_opcode,
            pcmem1,
            pcmem2,
            cpu.registers.a,
            if cpu.registers.f.zero { 'Z' } else { '-' },
            if cpu.registers.f.subtract { 'N' } else { '-' },
            if cpu.registers.f.half_carry { 'H' } else { '-' },
            if cpu.registers.f.carry { 'C' } else { '-' },
            cpu.registers.get_bc(),
            cpu.registers.get_de(),
            cpu.registers.get_hl(),
            cpu.bus.interrupt_controller.get_ie_register(),
            cpu.bus.interrupt_controller.get_int_flags()
        )
    } else {
        let pcmem0 = cpu.bus.read_byte(None, cpu.pc);
        let pcmem1 = cpu.bus.read_byte(None, cpu.pc.wrapping_add(1));
        let pcmem2 = cpu.bus.read_byte(None, cpu.pc.wrapping_add(2));
        let pcmem3 = cpu.bus.read_byte(None, cpu.pc.wrapping_add(3));
        format!(
            "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} SP:{:04X} PC:{:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
            cpu.registers.a,
            cpu.registers.f.as_byte(),
            cpu.registers.b,
            cpu.registers.c,
            cpu.registers.d,
            cpu.registers.e,
            cpu.registers.h,
            cpu.registers.l,
            cpu.sp,
            cpu.pc,
            pcmem0, pcmem1, pcmem2, pcmem3
        )
    };

    // Create logs directory if it doesn't exist
    if let Err(_) = std::fs::create_dir_all("logs") {
        return; // If we can't create the directory, skip logging
    }

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/cpu_log.txt")
    {
        let _ = file.write_all(log_entry.as_bytes());
    }
}
