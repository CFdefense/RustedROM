/**
 * CPU Operations Module - Game Boy CPU Instruction Implementations
 *
 * This module contains the actual implementation of all Game Boy CPU instructions.
 * Each function corresponds to a specific instruction type and handles the detailed
 * execution logic, register updates, memory access, and flag modifications.
 *
 * Instruction Categories Implemented:
 *
 * Bit Operations (CB-prefixed):
 * - Shifts: SRL, SRA, SLA (logical/arithmetic shifts)
 * - Rotates: RLC, RRC, RL, RR (with and without carry)
 * - SWAP: Exchange upper and lower nibbles
 * - BIT: Test specific bit and set zero flag
 * - RES/SET: Reset or set specific bit in register
 *
 * Arithmetic Operations:
 * - ADD/ADC: Addition with optional carry
 * - SUB/SBC: Subtraction with optional carry  
 * - INC/DEC: Increment/decrement 8-bit and 16-bit values
 * - DAA: Decimal adjust after BCD arithmetic
 * - CPL: Complement (invert) accumulator
 *
 * Logical Operations:
 * - AND/OR/XOR: Bitwise logical operations
 * - CP: Compare (subtract without storing result)
 *
 * Load Operations:
 * - LD: Comprehensive load instruction handling all addressing modes
 * - Supports register-to-register, immediate, and memory operations
 *
 * Control Flow:
 * - JP/JR: Absolute and relative jumps with conditions
 * - CALL/RET: Subroutine calls and returns with stack management
 * - RST: Restart vectors for interrupt handling
 *
 * Stack Operations:
 * - PUSH/POP: Stack manipulation for 16-bit register pairs
 *
 * Flag Management:
 * All arithmetic and logical operations properly update the CPU flags register:
 * - Zero (Z): Set when result is zero
 * - Subtract (N): Set for subtraction operations  
 * - Half Carry (H): Set when carry occurs from bit 3 to bit 4
 * - Carry (C): Set when carry occurs from bit 7 or borrow occurs
 *
 * The operations maintain cycle-accurate timing and authentic Game Boy behavior
 * for maximum compatibility with original software.
 */
use crate::hdw::cpu::*;
use crate::hdw::cpu_util::*;
use crate::hdw::emu::*;
use crate::hdw::instructions::*;
use crate::hdw::stack::*;
// [0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F]
pub fn op_srl(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let lsb = original_value & 0x1;
    let result = original_value >> 1;

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    // Write the result back to the target register or memory
    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    // Update Flags
    set_flags_after_pref_op(cpu, lsb, result);
}

// [0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37]
pub fn op_swap(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let result = (original_value << 4) | (original_value >> 4);

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_swap(cpu, result);
}

// [0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F]
pub fn op_sra(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let lsb = original_value & 0x1;
    let sign_bit = original_value & 0x80; // Preserve original sign bit
    let mut result = original_value >> 1;
    result |= sign_bit; // Ensure original sign bit is kept

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_pref_op(cpu, lsb, result);
}

// [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27]
pub fn op_sla(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let bit_7 = (original_value >> 7) & 0x1; // MSB for carry
    let result = original_value << 1;

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_pref_op(cpu, bit_7, result);
}

// [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]
pub fn op_rlc(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let bit_7 = (original_value >> 7) & 0x1; // MSB for carry and for rotating to bit 0
    let result = (original_value << 1) | bit_7;

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_pref_op(cpu, bit_7, result);
}

// [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F]
pub fn op_rrc(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let bit_0 = original_value & 0x1; // LSB for carry and for rotating to bit 7
    let result = (original_value >> 1) | (bit_0 << 7); // Corrected: bit_0 << 7

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_pref_op(cpu, bit_0, result);
}

// [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17]
pub fn op_rl(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let prev_carry = cpu.registers.f.carry as u8;
    let new_carry_val = (original_value >> 7) & 0x1; // MSB of original value becomes new carry
    let result = (original_value << 1) | prev_carry; // Old carry goes into LSB

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_pref_op(cpu, new_carry_val, result);
}

// [0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F]
pub fn op_rr(cpu: &mut CPU, target: HLTarget) {
    let original_value = match_hl(cpu, &target);
    let prev_carry = cpu.registers.f.carry as u8;
    let new_carry_val = original_value & 0x1; // LSB of original value becomes new carry
    let result = (original_value >> 1) | (prev_carry << 7); // Old carry goes into MSB

    if target != HLTarget::HL {
        emu_cycles(cpu, 1);
    }

    match target {
        HLTarget::A => cpu.registers.a = result,
        HLTarget::B => cpu.registers.b = result,
        HLTarget::C => cpu.registers.c = result,
        HLTarget::D => cpu.registers.d = result,
        HLTarget::E => cpu.registers.e = result,
        HLTarget::H => cpu.registers.h = result,
        HLTarget::L => cpu.registers.l = result,
        HLTarget::HL => cpu.bus.write_byte(cpu.registers.get_hl(), result),
    }

    set_flags_after_pref_op(cpu, new_carry_val, result);
}

// [0x2F]
pub fn op_cpl(cpu: &mut CPU) {
    // Flip all bits of register A
    cpu.registers.a = !cpu.registers.a;

    // Set flags
    set_flags_after_cpl(cpu);
}

// [0x27]
pub fn op_daa(cpu: &mut CPU) {
    let mut a_val = cpu.registers.a;
    let n_flag = cpu.registers.f.subtract; // N flag from previous operation
    let h_flag_old = cpu.registers.f.half_carry; // H flag from previous operation
    let c_flag_old = cpu.registers.f.carry; // C flag from previous operation

    let mut adjustment: u8 = 0x00;
    let mut set_new_carry_flag = false;

    if !n_flag {
        // Previous operation was an addition
        if c_flag_old || a_val > 0x99 {
            adjustment |= 0x60;
            set_new_carry_flag = true;
        }
        if h_flag_old || (a_val & 0x0F) > 0x09 {
            adjustment |= 0x06;
        }
        a_val = a_val.wrapping_add(adjustment);
    } else {
        // Previous operation was a subtraction
        if c_flag_old {
            adjustment |= 0x60;
            set_new_carry_flag = true; // If C was set by subtraction, DAA also sets C.
        }
        if h_flag_old {
            adjustment |= 0x06;
        }
        a_val = a_val.wrapping_sub(adjustment);
    }

    cpu.registers.a = a_val;

    // Update Flags: Z is based on new A, N is preserved, H is cleared, C is set by set_new_carry_flag.
    // The set_flags_after_daa function already handles Z, H, and C correctly based on its input.
    // N is not touched by set_flags_after_daa.
    set_flags_after_daa(cpu, set_new_carry_flag);
}

// [0x1F]
pub fn op_rra(cpu: &mut CPU) {
    // Store the original bit 0 to set the carry flag
    let bit_0 = cpu.registers.a & 1;

    // Rotate right: shift right by 1 and add carry to bit 7
    cpu.registers.a = (cpu.registers.a >> 1) | (cpu.registers.f.carry as u8) << 7;

    // Update Flags
    set_flags_after_no_pre_rl_rr(cpu, bit_0);
}

// [0x17]
pub fn op_rla(cpu: &mut CPU) {
    // Store the original bit 7 to set the carry flag
    let bit_7 = (cpu.registers.a & 0x80) >> 7;

    // Rotate left: shift left by 1 and add carry to bit 0
    cpu.registers.a = (cpu.registers.a << 1) | (cpu.registers.f.carry as u8);

    // Update Flags
    set_flags_after_no_pre_rl_rr(cpu, bit_7);
}

// [0x0F]
pub fn op_rrca(cpu: &mut CPU) {
    // Store the original bit 0 to set the carry flag and bit 7
    let bit_0 = cpu.registers.a & 1;

    // Rotate right: shift right by 1 and add bit 0 to bit 7
    cpu.registers.a = (cpu.registers.a >> 1) | (bit_0 << 7);

    // Update Flags
    set_flags_after_no_pre_rl_rr(cpu, bit_0);
}
// [0x07]

pub fn op_rlca(cpu: &mut CPU) {
    // Store the original bit 7 to set the Carry flag and bit 0
    let bit_7 = (cpu.registers.a >> 7) & 1;

    // Rotate left: shift left by 1 and add bit 7 to bit 0
    cpu.registers.a = (cpu.registers.a << 1) | bit_7;

    // Update Flags
    set_flags_after_no_pre_rl_rr(cpu, bit_7);
}

/*
[0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F,
 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F,
 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F]

*/
pub fn op_bit(cpu: &mut CPU, target: ByteTarget) {
    let bit_mask: u8;
    let target_register_value: u8;
    match target {
        // [0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]
        ByteTarget::Zero(hl_target) => {
            // BIT 0, r
            bit_mask = 1 << 0;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F]
        ByteTarget::One(hl_target) => {
            // BIT 1, r
            bit_mask = 1 << 1;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57]
        ByteTarget::Two(hl_target) => {
            // BIT 2, r
            bit_mask = 1 << 2;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F]
        ByteTarget::Three(hl_target) => {
            // BIT 3, r
            bit_mask = 1 << 3;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67]
        ByteTarget::Four(hl_target) => {
            // BIT 4, r
            bit_mask = 1 << 4;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F]
        ByteTarget::Five(hl_target) => {
            // BIT 5, r
            bit_mask = 1 << 5;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77]
        ByteTarget::Six(hl_target) => {
            // BIT 6, r
            bit_mask = 1 << 6;
            target_register_value = match_hl(cpu, &hl_target);
        }
        // [0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F]
        ByteTarget::Seven(hl_target) => {
            // BIT 7, r
            bit_mask = 1 << 7;
            target_register_value = match_hl(cpu, &hl_target);
        }
    }

    // Set Flags
    set_flags_after_bit(cpu, bit_mask, target_register_value);
}

/*
[0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F,
 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F,
 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF]
*/
pub fn op_res(cpu: &mut CPU, target: ByteTarget) {
    let mask: u8;
    let target_register: u8;
    let is_mem: bool;
    let found_target: HLTarget;

    match target {
        // [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87]
        ByteTarget::Zero(hl_target) => {
            mask = 0b11111110; // Byte Mask
            found_target = hl_target;
        }
        // [0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F]
        ByteTarget::One(hl_target) => {
            mask = 0b11111101; // Byte Mask
            found_target = hl_target;
        }
        // [0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97]
        ByteTarget::Two(hl_target) => {
            mask = 0b11111011; // Byte Mask
            found_target = hl_target;
        }
        // [0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F]
        ByteTarget::Three(hl_target) => {
            mask = 0b11110111; // Byte Mask
            found_target = hl_target;
        }
        // [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7]
        ByteTarget::Four(hl_target) => {
            mask = 0b11101111; // Byte Mask
            found_target = hl_target;
        }
        // [0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF]
        ByteTarget::Five(hl_target) => {
            mask = 0b11011111; // Byte Mask
            found_target = hl_target;
        }
        // [0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7]
        ByteTarget::Six(hl_target) => {
            mask = 0b10111111; // Byte Mask
            found_target = hl_target;
        }
        // [0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF]
        ByteTarget::Seven(hl_target) => {
            mask = 0b01111111; // Byte Mask
            found_target = hl_target;
        }
    }

    is_mem = matches!(found_target, HLTarget::HL);

    // Get Target Register
    target_register = match_hl(cpu, &found_target);

    // Perform Operation
    if is_mem {
        // if we're updating memory write back to grabbed location the new value
        cpu.bus
            .write_byte(cpu.registers.get_hl(), target_register & mask);
    } else {
        // Update the appropriate register based on found_target
        match found_target {
            HLTarget::A => cpu.registers.a &= mask,
            HLTarget::B => cpu.registers.b &= mask,
            HLTarget::C => cpu.registers.c &= mask,
            HLTarget::D => cpu.registers.d &= mask,
            HLTarget::E => cpu.registers.e &= mask,
            HLTarget::H => cpu.registers.h &= mask,
            HLTarget::L => cpu.registers.l &= mask,
            HLTarget::HL => {} // Already handled in is_mem case
        }
    }
}

/*
[0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF
 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF
 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF
 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF]
*/
pub fn op_set(cpu: &mut CPU, target: ByteTarget) {
    let mask: u8;
    let is_mem: bool;
    let found_target: HLTarget;

    match target {
        // [0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7]
        ByteTarget::Zero(hl_target) => {
            mask = 0b00000001; // Byte Mask
            found_target = hl_target;
        }
        // [0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF]
        ByteTarget::One(hl_target) => {
            mask = 0b00000010;
            found_target = hl_target;
        }
        // [0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7]
        ByteTarget::Two(hl_target) => {
            mask = 0b00000100;
            found_target = hl_target;
        }
        // [0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF]
        ByteTarget::Three(hl_target) => {
            mask = 0b00001000;
            found_target = hl_target;
        }
        // [0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7]
        ByteTarget::Four(hl_target) => {
            mask = 0b00010000;
            found_target = hl_target;
        }
        // [0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF]
        ByteTarget::Five(hl_target) => {
            mask = 0b00100000;
            found_target = hl_target;
        }
        // [0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7]
        ByteTarget::Six(hl_target) => {
            mask = 0b01000000;
            found_target = hl_target;
        }
        // [0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF]
        ByteTarget::Seven(hl_target) => {
            mask = 0b10000000;
            found_target = hl_target;
        }
    }

    // Determine if we're using memory
    is_mem = matches!(found_target, HLTarget::HL);

    if is_mem {
        // If we're updating memory, read current value and set the bit
        let value = cpu.bus.read_byte(None, cpu.registers.get_hl());
        cpu.bus.write_byte(cpu.registers.get_hl(), value | mask);
    } else {
        // Update the appropriate register based on found_target
        match found_target {
            HLTarget::A => cpu.registers.a |= mask,
            HLTarget::B => cpu.registers.b |= mask,
            HLTarget::C => cpu.registers.c |= mask,
            HLTarget::D => cpu.registers.d |= mask,
            HLTarget::E => cpu.registers.e |= mask,
            HLTarget::H => cpu.registers.h |= mask,
            HLTarget::L => cpu.registers.l |= mask,
            HLTarget::HL => {} // Already handled in is_mem case
        }
    }
}

// [0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xFE]
pub fn op_cp(cpu: &mut CPU, target: OPTarget) {
    match target {
        OPTarget::B => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.b);
        } // [0xB8]
        OPTarget::C => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.c);
        } // [0xB9]
        OPTarget::D => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.d);
        } // [0xBA]
        OPTarget::E => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.e);
        } // [0xBB]
        OPTarget::H => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.h);
        } // [0xBC]
        OPTarget::L => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.l);
        } // [0xBD]
        OPTarget::A => {
            set_flags_after_cp(cpu, cpu.registers.a, cpu.registers.a);
        } // [0xBF]
        // [0xBE]
        OPTarget::HL => {
            let hl_value = cpu.bus.read_byte(None, cpu.registers.get_hl());
            set_flags_after_cp(cpu, cpu.registers.a, hl_value);
        }
        // [0xFE]
        OPTarget::D8 => {
            let d8_value = cpu.bus.read_byte(None, cpu.pc + 1);
            set_flags_after_cp(cpu, cpu.registers.a, d8_value);
            cpu.pc = cpu.pc.wrapping_add(1);
        }
    }
}

// [0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xF6]
pub fn op_or(cpu: &mut CPU, target: OPTarget) {
    match target {
        OPTarget::B => {
            cpu.registers.a |= cpu.registers.b;
        } // [0xB0]
        OPTarget::C => {
            cpu.registers.a |= cpu.registers.c;
        } // [0xB1]
        OPTarget::D => {
            cpu.registers.a |= cpu.registers.d;
        } // [0xB2]
        OPTarget::E => {
            cpu.registers.a |= cpu.registers.e;
        } // [0xB3]
        OPTarget::H => {
            cpu.registers.a |= cpu.registers.h;
        } // [0xB4]
        OPTarget::L => {
            cpu.registers.a |= cpu.registers.l;
        } // [0xB5]
        OPTarget::A => {
            cpu.registers.a |= cpu.registers.a;
        } // [0xB7]
        // [0xB6]
        OPTarget::HL => {
            cpu.registers.a |= cpu.bus.read_byte(None, cpu.registers.get_hl());
        }
        // [0xF6]
        OPTarget::D8 => {
            cpu.registers.a |= cpu.bus.read_byte(None, cpu.pc + 1);
            cpu.pc = cpu.pc.wrapping_add(1);
        }
    }
    // Set Flags
    set_flags_after_xor_or(cpu, cpu.registers.a);
}

// [0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xEE]
pub fn op_xor(cpu: &mut CPU, target: OPTarget) {
    match target {
        OPTarget::B => {
            cpu.registers.a ^= cpu.registers.b;
        } // [0xA8]
        OPTarget::C => {
            cpu.registers.a ^= cpu.registers.c;
        } // [0xA9]
        OPTarget::D => {
            cpu.registers.a ^= cpu.registers.d;
        } // [0xAA]
        OPTarget::E => {
            cpu.registers.a ^= cpu.registers.e;
        } // [0xAB]
        OPTarget::H => {
            cpu.registers.a ^= cpu.registers.h;
        } // [0xAC]
        OPTarget::L => {
            cpu.registers.a ^= cpu.registers.l;
        } // [0xAD]
        OPTarget::A => {
            cpu.registers.a ^= cpu.registers.a;
        } // [0xAF]
        // [0xAE]
        OPTarget::HL => {
            cpu.registers.a ^= cpu.bus.read_byte(None, cpu.registers.get_hl());
        }
        // [0xEE]
        OPTarget::D8 => {
            cpu.registers.a ^= cpu.bus.read_byte(None, cpu.pc + 1);
            cpu.pc = cpu.pc.wrapping_add(1);
        }
    }
    // Set Flags
    set_flags_after_xor_or(cpu, cpu.registers.a);
}

// [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xE6]
pub fn op_and(cpu: &mut CPU, target: OPTarget) {
    match target {
        OPTarget::B => {
            cpu.registers.a &= cpu.registers.b;
        } // [0xA0]
        OPTarget::C => {
            cpu.registers.a &= cpu.registers.c;
        } // [0xA1]
        OPTarget::D => {
            cpu.registers.a &= cpu.registers.d;
        } // [0xA2]
        OPTarget::E => {
            cpu.registers.a &= cpu.registers.e;
        } // [0xA3]
        OPTarget::H => {
            cpu.registers.a &= cpu.registers.h;
        } // [0xA4]
        OPTarget::L => {
            cpu.registers.a &= cpu.registers.l;
        } // [0xA5]
        OPTarget::A => {
            cpu.registers.a &= cpu.registers.a;
        } // [0xA7]
        // [0xA6]
        OPTarget::HL => {
            cpu.registers.a &= cpu.bus.read_byte(None, cpu.registers.get_hl());
        }
        // [0xE6]
        OPTarget::D8 => {
            cpu.registers.a &= cpu.bus.read_byte(None, cpu.pc + 1);
        }
    }
    // Set Flags
    set_flags_after_and(cpu, cpu.registers.a);
}

// [0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xDE]
pub fn op_sbc(cpu: &mut CPU, target: OPTarget) {
    let original_value = cpu.registers.a;
    let carry_in = cpu.registers.f.carry as u8;

    match target {
        OPTarget::B => {
            let operand_value = cpu.registers.b;
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::C => {
            let operand_value = cpu.registers.c;
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::D => {
            let operand_value = cpu.registers.d;
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
            // Increment the program counter -- NO, this is handled by main loop for 1-byte opcodes
            // cpu.pc = cpu.pc.wrapping_add(1);
        }
        OPTarget::E => {
            let operand_value = cpu.registers.e;
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::H => {
            let operand_value = cpu.registers.h;
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::L => {
            let operand_value = cpu.registers.l;
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::HL => {
            let operand_value = cpu.bus.read_byte(None, cpu.registers.get_hl());
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::A => {
            let operand_value = original_value; // SBC A, A
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
        }
        OPTarget::D8 => {
            let operand_value = cpu.bus.read_byte(None, cpu.pc + 1);
            cpu.registers.a = original_value
                .wrapping_sub(operand_value)
                .wrapping_sub(carry_in);
            set_flags_after_sbc(
                cpu,
                cpu.registers.a,
                original_value,
                operand_value,
                carry_in,
            );
            cpu.pc = cpu.pc.wrapping_add(1); // Increment for the d8 operand
        }
    }
}

// [0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0xD6]
pub fn op_sub(cpu: &mut CPU, target: OPTarget) {
    // Get Original Value
    let original_value = cpu.registers.a;
    match target {
        // [0x90]
        OPTarget::B => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.b);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, cpu.registers.b);
        }
        // [0x91]
        OPTarget::C => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.c);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, cpu.registers.c);
        }
        // [0x92]
        OPTarget::D => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.d);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, cpu.registers.d);
        }
        // [0x93]
        OPTarget::E => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.e);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, cpu.registers.e);
        }
        // [0x94]
        OPTarget::H => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.h);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, cpu.registers.h);
        }
        // [0x95]
        OPTarget::L => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.l);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, cpu.registers.l);
        }
        // [0x96]
        OPTarget::HL => {
            // SUB
            let hl_value = cpu.bus.read_byte(None, cpu.registers.get_hl());
            cpu.registers.a = cpu.registers.a.wrapping_sub(hl_value);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, hl_value);
        }
        // [0x97]
        OPTarget::A => {
            // SUB
            cpu.registers.a = cpu.registers.a.wrapping_sub(cpu.registers.a);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, original_value);
        }
        // [0xD6]
        OPTarget::D8 => {
            // SUB
            let d8_value = cpu.bus.read_byte(None, cpu.pc + 1);
            cpu.registers.a = cpu.registers.a.wrapping_sub(d8_value);

            // Set Flags
            set_flags_after_sub(cpu, cpu.registers.a, original_value, d8_value);
            cpu.pc = cpu.pc.wrapping_add(1);
        }
    }
}

// [0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0xCE]
pub fn op_adc(cpu: &mut CPU, target: OPTarget) {
    let original_a = cpu.registers.a; // Store Original A
    let carry_in = cpu.registers.f.carry as u8; // Carry that will be part of the sum

    match target {
        // [0x88]
        OPTarget::B => {
            let val = cpu.registers.b;
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x89]
        OPTarget::C => {
            let val = cpu.registers.c;
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x8A]
        OPTarget::D => {
            let val = cpu.registers.d;
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x8B]
        OPTarget::E => {
            let val = cpu.registers.e;
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x8C]
        OPTarget::H => {
            let val = cpu.registers.h;
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x8D]
        OPTarget::L => {
            let val = cpu.registers.l;
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x8E]
        OPTarget::HL => {
            let val = cpu.bus.read_byte(None, cpu.registers.get_hl());
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0x8F]
        OPTarget::A => {
            let val = original_a; // A itself is the operand
            cpu.registers.a = original_a.wrapping_add(val).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, val);
        }
        // [0xCE]
        OPTarget::D8 => {
            let d8_value = cpu.bus.read_byte(None, cpu.pc + 1);
            cpu.registers.a = original_a.wrapping_add(d8_value).wrapping_add(carry_in);
            set_flags_after_adc(cpu, cpu.registers.a, original_a, d8_value);
            cpu.pc = cpu.pc.wrapping_add(1); // INC PC due to Byte Read
        }
    }
}

// [0x09, 0x19, 0x29, 0x39, 0x80-0x87, 0xC6, 0xE8]
pub fn op_add(cpu: &mut CPU, target: OPType) {
    match target {
        // [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87] // ADD A, r
        OPType::LoadA(hl_target_enum_val) => {
            let reg_operand = match_hl(cpu, &hl_target_enum_val);
            let original_a = cpu.registers.a;
            cpu.registers.a = original_a.wrapping_add(reg_operand);
            set_flags_after_add_a(cpu, reg_operand, original_a, false);
        }
        // [0x09, 0x19, 0x29, 0x39] // ADD HL, rr
        OPType::LoadHL(add_n16_target_enum_val) => {
            let original_hl = cpu.registers.get_hl();
            let value_to_add = match_n16(cpu, add_n16_target_enum_val.clone());
            cpu.registers.set_hl(original_hl.wrapping_add(value_to_add));
            set_flags_after_add_n16(cpu, original_hl, value_to_add);

            if add_n16_target_enum_val == AddN16Target::HL {
                emu_cycles(cpu, 1);
            }
        }
        // [0xE8] // ADD SP, e8
        OPType::LoadSP => {
            let original_sp = cpu.sp;
            let r8_signed = cpu.bus.read_byte(None, cpu.pc + 1) as i8;

            // Perform addition: SP = SP + r8_signed
            cpu.sp = (original_sp as i32 + r8_signed as i32) as u16;

            // Set Flags using the correct function
            set_flags_after_add_sp_r8(cpu, original_sp, r8_signed);

            // INC PC for opcode + r8
            cpu.pc = cpu.pc.wrapping_add(1);
        }
        // [0xC6] // ADD A, d8
        OPType::LoadD8 => {
            let immediate_operand: u8 = cpu.bus.read_byte(None, cpu.pc + 1);
            let original_a = cpu.registers.a;
            cpu.registers.a = original_a.wrapping_add(immediate_operand);
            set_flags_after_add_a(cpu, immediate_operand, original_a, true);
            cpu.pc = cpu.pc.wrapping_add(1);
        }
    }
}

/*
[0x01, 0x11, 0x21, 0x31,
 0x02, 0x12, 0x22, 0x32,
 0x06, 0x16, 0x26, 0x36,
 0x0A, 0x1A, 0x2A, 0x3A,
 0x0E, 0x1E, 0x2E, 0x3E
 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F,
 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67,
 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F,
 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x77,
 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F
 0xE0, 0xF0, 0xE2, 0xF2, 0x08, 0xF8, 0xF9, 0xEA, 0xFA]
*/
pub fn op_ld(cpu: &mut CPU, target: LoadType) {
    match target {
        LoadType::RegInReg(target, source) => match target {
            // [0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]
            HLTarget::B => match source {
                // [0x40]
                HLTarget::B => {
                    // LD B,B (no-op)
                }
                // [0x41]
                HLTarget::C => {
                    cpu.registers.b = cpu.registers.c;
                }
                // [0x42]
                HLTarget::D => {
                    cpu.registers.b = cpu.registers.d;
                }
                // [0x43]
                HLTarget::E => {
                    cpu.registers.b = cpu.registers.e;
                }
                // [0x44]
                HLTarget::H => {
                    cpu.registers.b = cpu.registers.h;
                }
                // [0x45]
                HLTarget::L => {
                    cpu.registers.b = cpu.registers.l;
                }
                // [0x46]
                HLTarget::HL => {
                    cpu.registers.b = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // 0x47
                HLTarget::A => {
                    cpu.registers.b = cpu.registers.a;
                }
            },
            // [0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F]
            HLTarget::C => match source {
                // [0x48]
                HLTarget::B => {
                    cpu.registers.c = cpu.registers.b;
                }
                // [0x49]
                HLTarget::C => {
                    // LD C,C (no-op)
                }
                // [0x4A]
                HLTarget::D => {
                    cpu.registers.c = cpu.registers.d;
                }
                // [0x4B]
                HLTarget::E => {
                    cpu.registers.c = cpu.registers.e;
                }
                // [0x4C]
                HLTarget::H => {
                    cpu.registers.c = cpu.registers.h;
                }
                // [0x4D]
                HLTarget::L => {
                    cpu.registers.c = cpu.registers.l;
                }
                // [0x4E]
                HLTarget::HL => {
                    cpu.registers.c = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // [0x4F]
                HLTarget::A => {
                    cpu.registers.c = cpu.registers.a;
                }
            },
            // [0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57]
            HLTarget::D => match source {
                // [0x50]
                HLTarget::B => {
                    cpu.registers.d = cpu.registers.b;
                }
                // [0x51]
                HLTarget::C => {
                    cpu.registers.d = cpu.registers.c;
                }
                // [0x52]
                HLTarget::D => {
                    // LD D,D (no-op)
                }
                // [0x53]
                HLTarget::E => {
                    cpu.registers.d = cpu.registers.e;
                }
                // [0x54]
                HLTarget::H => {
                    cpu.registers.d = cpu.registers.h;
                }
                // [0x55]
                HLTarget::L => {
                    cpu.registers.d = cpu.registers.l;
                }
                // [0x56]
                HLTarget::HL => {
                    cpu.registers.d = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // [0x57]
                HLTarget::A => {
                    cpu.registers.d = cpu.registers.a;
                }
            },
            // [0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F]
            HLTarget::E => match source {
                // [0x58]
                HLTarget::B => {
                    cpu.registers.e = cpu.registers.b;
                }
                // [0x59]
                HLTarget::C => {
                    cpu.registers.e = cpu.registers.c;
                }
                // [0x5A]
                HLTarget::D => {
                    cpu.registers.e = cpu.registers.d;
                }
                // [0x5B]
                HLTarget::E => {
                    // LD E,E (no-op)
                }
                // [0x5C]
                HLTarget::H => {
                    cpu.registers.e = cpu.registers.h;
                }
                // [0x5D]
                HLTarget::L => {
                    cpu.registers.e = cpu.registers.l;
                }
                // [0x5E]
                HLTarget::HL => {
                    cpu.registers.e = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // [0x5F]
                HLTarget::A => {
                    cpu.registers.e = cpu.registers.a;
                }
            },
            // [0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67]
            HLTarget::H => match source {
                // [0x60]
                HLTarget::B => {
                    cpu.registers.h = cpu.registers.b;
                }
                // [0x61]
                HLTarget::C => {
                    cpu.registers.h = cpu.registers.c;
                }
                // [0x62]
                HLTarget::D => {
                    cpu.registers.h = cpu.registers.d;
                }
                // [0x63]
                HLTarget::E => {
                    cpu.registers.h = cpu.registers.e;
                }
                // [0x64]
                HLTarget::H => {
                    // LD H,H (no-op)
                }
                // [0x65]
                HLTarget::L => {
                    cpu.registers.h = cpu.registers.l;
                }
                // [0x66]
                HLTarget::HL => {
                    cpu.registers.h = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // [0x67]
                HLTarget::A => {
                    cpu.registers.h = cpu.registers.a;
                }
            },
            // [0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F]
            HLTarget::L => match source {
                // [0x68]
                HLTarget::B => {
                    cpu.registers.l = cpu.registers.b;
                }
                // [0x69]
                HLTarget::C => {
                    cpu.registers.l = cpu.registers.c;
                }
                // [0x6A]
                HLTarget::D => {
                    cpu.registers.l = cpu.registers.d;
                }
                // [0x6B]
                HLTarget::E => {
                    cpu.registers.l = cpu.registers.e;
                }
                // [0x6C]
                HLTarget::H => {
                    cpu.registers.l = cpu.registers.h;
                }
                // [0x6D]
                HLTarget::L => {
                    // LD L,L (no-op)
                }
                // [0x6E]
                HLTarget::HL => {
                    cpu.registers.l = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // [0x6F]
                HLTarget::A => {
                    cpu.registers.l = cpu.registers.a;
                }
            },
            // [0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x77]
            HLTarget::HL => {
                emu_cycles(cpu, 1);
                match source {
                    // [0x70]
                    HLTarget::B => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.b);
                    }
                    // [0x71]
                    HLTarget::C => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.c);
                    }
                    // [0x72]
                    HLTarget::D => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.d);
                    }
                    // [0x73]
                    HLTarget::E => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.e);
                    }
                    // [0x74]
                    HLTarget::H => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.h);
                    }
                    // [0x75]
                    HLTarget::L => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.l);
                    }
                    // [0x77]
                    HLTarget::A => {
                        cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.a);
                    }
                    _ => panic!("Getting LD HL HL Should be HALT"),
                }
            }
            // [0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F]
            HLTarget::A => match source {
                // [0x78]
                HLTarget::B => {
                    cpu.registers.a = cpu.registers.b;
                }
                // [0x79]
                HLTarget::C => {
                    cpu.registers.a = cpu.registers.c;
                }
                // [0x7A]
                HLTarget::D => {
                    cpu.registers.a = cpu.registers.d;
                }
                // [0x7B]
                HLTarget::E => {
                    cpu.registers.a = cpu.registers.e;
                }
                // [0x7C]
                HLTarget::H => {
                    cpu.registers.a = cpu.registers.h;
                }
                // [0x7D]
                HLTarget::L => {
                    cpu.registers.a = cpu.registers.l;
                }
                // [0x7E]
                HLTarget::HL => {
                    cpu.registers.a = cpu.bus.read_byte(None, cpu.registers.get_hl());
                }
                // [0x7F]
                HLTarget::A => {
                    // LD A,A (no-op)
                }
            },
        },
        // [0x01, 0x21, 0xF8, 0x11, 0x08]
        LoadType::Word(target, source) => {
            // Read the next two bytes from bus at the current PC
            let low_byte = cpu.bus.read_byte(None, cpu.pc + 1); // Read the low byte
            let high_byte = cpu.bus.read_byte(None, cpu.pc + 2); // Read the high byte

            // Combine the low and high bytes into a 16-bit value
            let word_value = ((high_byte as u16) << 8) | (low_byte as u16);

            match target {
                // [0x01]
                LoadWordTarget::BC => match source {
                    LoadWordSource::N16 => {
                        cpu.registers.set_bc(word_value as u16);
                        cpu.pc = cpu.pc.wrapping_add(2);
                    }
                    _ => panic!("LD WORD BAD MATCH"),
                },
                // [0x21, 0xF8]
                LoadWordTarget::HL => match source {
                    // [0x21]
                    LoadWordSource::N16 => {
                        cpu.registers.set_hl(word_value as u16);

                        cpu.pc = cpu.pc.wrapping_add(2);
                    }
                    // [0xF8]
                    LoadWordSource::SPE8 => {
                        let r8_signed = cpu.bus.read_byte(None, cpu.pc + 1) as i8;
                        let original_sp = cpu.sp;

                        let result_hl = (original_sp as i32 + r8_signed as i32) as u16;
                        cpu.registers.set_hl(result_hl);

                        set_flags_after_ld_spe8(cpu, original_sp, r8_signed);

                        cpu.pc = cpu.pc.wrapping_add(1);
                    }
                    _ => panic!("LD WORD BAD MATCH"),
                },
                // [0x11]
                LoadWordTarget::DE => match source {
                    LoadWordSource::N16 => {
                        cpu.registers.set_de(word_value as u16);
                        cpu.pc = cpu.pc.wrapping_add(2);
                    }
                    _ => panic!("LD WORD BAD MATCH"),
                },
                // [0x08]
                LoadWordTarget::N16 => match source {
                    LoadWordSource::SP => {
                        cpu.bus.write_byte(word_value, (cpu.sp & 0x00FF) as u8);
                        cpu.bus.write_byte(word_value + 1, (cpu.sp >> 8) as u8);
                        cpu.pc = cpu.pc.wrapping_add(2);
                    }
                    _ => panic!("LD WORD BAD MATCH"),
                },
                // [0x31, 0xF9]
                LoadWordTarget::SP => match source {
                    // [0xF9]
                    LoadWordSource::HL => {
                        cpu.sp = cpu.registers.get_hl();
                    }
                    // [0x31]
                    LoadWordSource::N16 => {
                        //emu_cycles(cpu, 2); // 2 cycle for the LD
                        cpu.sp = word_value;
                        cpu.pc = cpu.pc.wrapping_add(2);
                    }
                    _ => panic!("LD WORD BAD MATCH"),
                },
            }
        }
        // [0x02, 0x12, 0x22, 0x32]
        LoadType::AStoreInN16(target) => {
            emu_cycles(cpu, 1);
            match target {
                // [0x02]
                LoadN16::BC => {
                    cpu.bus.write_byte(cpu.registers.get_bc(), cpu.registers.a);
                }
                // [0x12]
                LoadN16::DE => {
                    cpu.bus.write_byte(cpu.registers.get_de(), cpu.registers.a);
                }
                // [0x22]
                LoadN16::HLINC => {
                    cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.a);
                    cpu.registers.set_hl(cpu.registers.get_hl().wrapping_add(1));
                }
                // [0x32]
                LoadN16::HLDEC => {
                    cpu.bus.write_byte(cpu.registers.get_hl(), cpu.registers.a);
                    cpu.registers.set_hl(cpu.registers.get_hl().wrapping_sub(1));
                }
            }
        }
        // [0x0A, 0x1A, 0x2A, 0x3A]
        LoadType::N16StoreInA(source) => match source {
            // [0x0A]
            LoadN16::BC => {
                cpu.registers.a = cpu.bus.read_byte(None, cpu.registers.get_bc());
            }
            // [0x1A]
            LoadN16::DE => {
                cpu.registers.a = cpu.bus.read_byte(None, cpu.registers.get_de());
            }
            // [0x3A]
            LoadN16::HLDEC => {
                cpu.registers.a = cpu.bus.read_byte(None, cpu.registers.get_hl());
                cpu.registers.set_hl(cpu.registers.get_hl().wrapping_sub(1));
            }
            // [0x2A]
            LoadN16::HLINC => {
                cpu.registers.a = cpu.bus.read_byte(None, cpu.registers.get_hl());
                cpu.registers.set_hl(cpu.registers.get_hl().wrapping_add(1));
            }
        },
        // [0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x36, 0x3E]
        LoadType::D8StoreInReg(target) => match target {
            // [0x06]
            HLTarget::B => {
                cpu.registers.b = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x0E]
            HLTarget::C => {
                cpu.registers.c = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x16]
            HLTarget::D => {
                cpu.registers.d = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x1E]
            HLTarget::E => {
                cpu.registers.e = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x26]
            HLTarget::H => {
                cpu.registers.h = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x2E]
            HLTarget::L => {
                cpu.registers.l = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x36]
            HLTarget::HL => {
                let d8_value = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.bus.write_byte(cpu.registers.get_hl(), d8_value);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
            // [0x3E]
            HLTarget::A => {
                cpu.registers.a = cpu.bus.read_byte(None, cpu.pc + 1);
                cpu.pc = cpu.pc.wrapping_add(1);
            }
        },
        // [0xE0, 0xF0]
        LoadType::AWithA8(target) => match target {
            // [0xF0]
            LoadA8Target::A => {
                // First read all values we need
                let address = 0xFF00 + cpu.bus.read_byte(None, cpu.pc + 1) as u16;

                // Then read the value at the calculated address
                // We create a temporary mutable reference to cpu for the read_byte call
                let value = {
                    let cpu_ref = cpu as *mut CPU;
                    // SAFETY: We're only creating a temporary reference and not modifying any state
                    // The CPU reference is valid for the duration of this scope
                    // We ensure no other mutable references exist during this time
                    cpu.bus.read_byte(Some(unsafe { &mut *cpu_ref }), address)
                };

                // Finally update register and INC PC due to Byte Read
                cpu.registers.a = value;
                cpu.pc = cpu.pc.wrapping_add(1);
                emu_cycles(cpu, 1);
            }
            // [0xE0]
            LoadA8Target::A8 => {
                // First read all values we need
                let address = 0xFF00 + cpu.bus.read_byte(None, cpu.pc + 1) as u16;
                let value = cpu.registers.a;

                cpu.bus.write_byte(address, value);

                // INC PC due to Byte Read
                cpu.pc = cpu.pc.wrapping_add(1);
                emu_cycles(cpu, 1);
            }
        },
        // [0xEA, 0xFA]
        LoadType::AWithA16(target) => {
            let low_byte = cpu.bus.read_byte(None, cpu.pc + 1); // Read the low byte
            let high_byte = cpu.bus.read_byte(None, cpu.pc + 2); // Read the high byte

            // Combine the low and high bytes into a 16-bit value
            let address = ((high_byte as u16) << 8) | (low_byte as u16);

            match target {
                // [0xFA]
                LoadA16Target::A => {
                    cpu.registers.a = cpu.bus.read_byte(None, address);
                    cpu.pc = cpu.pc.wrapping_add(2);
                }
                // [0xEA]
                LoadA16Target::A16 => {
                    cpu.bus.write_byte(address, cpu.registers.a);
                    cpu.pc = cpu.pc.wrapping_add(2);
                    emu_cycles(cpu, 1);
                }
            }
        }
        // [0xE2, 0xF2]
        LoadType::AWithAC(target) => match target {
            // [0xE2]
            LoadACTarget::C => {
                cpu.bus
                    .write_byte(0xFF00 + cpu.registers.c as u16, cpu.registers.a);
            }
            // [0xF2]
            LoadACTarget::A => {
                cpu.registers.a = cpu.bus.read_byte(None, 0xFF00 + cpu.registers.c as u16);
            }
        },
    }
}

// [0x05, 0x0B, 0x0D, 0x15, 0x1B, 0x1D, 0x25, 0x2B, 0x2D, 0x35, 0x3B, 0x3D]
pub fn op_dec(cpu: &mut CPU, target: AllRegisters) {
    match target {
        // Increment 8-bit registers and Set Flags
        // [0x3D]
        AllRegisters::A => {
            let original_value = cpu.registers.a;
            cpu.registers.a = cpu.registers.a.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.a, original_value);
        }
        // [0x05]
        AllRegisters::B => {
            let original_value = cpu.registers.b;
            cpu.registers.b = cpu.registers.b.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.b, original_value);
        }
        // [0x0D]
        AllRegisters::C => {
            let original_value = cpu.registers.c;
            cpu.registers.c = cpu.registers.c.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.c, original_value);
        }
        // [0x15]
        AllRegisters::D => {
            let original_value = cpu.registers.d;
            cpu.registers.d = cpu.registers.d.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.d, original_value);
        }
        // [0x1D]
        AllRegisters::E => {
            let original_value = cpu.registers.e;
            cpu.registers.e = cpu.registers.e.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.e, original_value);
        }
        // [0x25]
        AllRegisters::H => {
            let original_value = cpu.registers.h;
            cpu.registers.h = cpu.registers.h.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.h, original_value);
        }
        // [0x2D]
        AllRegisters::L => {
            let original_value = cpu.registers.l;
            cpu.registers.l = cpu.registers.l.wrapping_sub(1);
            set_flags_after_dec(cpu, cpu.registers.l, original_value);
        }

        // [0x35]
        AllRegisters::HLMEM => {
            // Increment value at bus location HL
            let hl_addr = cpu.registers.get_hl();
            let original_value = cpu.bus.read_byte(None, hl_addr);
            let value = original_value.wrapping_sub(1);
            cpu.bus.write_byte(hl_addr, value);
            set_flags_after_dec(cpu, value, original_value);
            emu_cycles(cpu, 1);
        }
        // 16-bit register increments (don't need to Set Flags for these)
        // [0x0B]
        AllRegisters::BC => {
            let new_bc = cpu.registers.get_bc().wrapping_sub(1);
            cpu.registers.set_bc(new_bc);
            emu_cycles(cpu, 1);
        }
        // [0x1B]
        AllRegisters::DE => {
            let new_de = cpu.registers.get_de().wrapping_sub(1);
            cpu.registers.set_de(new_de);
            emu_cycles(cpu, 1);
        }
        // [0x2B]
        AllRegisters::HL => {
            let new_hl = cpu.registers.get_hl().wrapping_sub(1);
            cpu.registers.set_hl(new_hl);
            emu_cycles(cpu, 1);
        }
        // [0x3B]
        AllRegisters::SP => {
            cpu.sp = cpu.sp.wrapping_sub(1);
            emu_cycles(cpu, 1);
        }
    }
}

// [0x03, 0x04, 0x0C, 0x13, 0x14, 0x1C, 0x23, 0x24, 0x2C, 0x33, 0x34, 0x3C]
pub fn op_inc(cpu: &mut CPU, target: AllRegisters) {
    match target {
        // Increment 8-bit registers and Set Flags
        // [0x3C]
        AllRegisters::A => {
            cpu.registers.a = cpu.registers.a.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.a);
        }
        // [0x04]
        AllRegisters::B => {
            cpu.registers.b = cpu.registers.b.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.b);
        }
        // [0x0C]
        AllRegisters::C => {
            cpu.registers.c = cpu.registers.c.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.c);
        }
        // [0x14]
        AllRegisters::D => {
            cpu.registers.d = cpu.registers.d.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.d);
        }
        // [0x1C]
        AllRegisters::E => {
            cpu.registers.e = cpu.registers.e.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.e);
        }
        // [0x24]
        AllRegisters::H => {
            cpu.registers.h = cpu.registers.h.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.h);
        }
        // [0x2C]
        AllRegisters::L => {
            cpu.registers.l = cpu.registers.l.wrapping_add(1);
            set_flags_after_inc(cpu, cpu.registers.l);
        }
        // [0x34]
        AllRegisters::HLMEM => {
            // Increment value at bus location HL
            let hl_addr = cpu.registers.get_hl();
            let value = cpu.bus.read_byte(None, hl_addr).wrapping_add(1);
            cpu.bus.write_byte(hl_addr, value);
            set_flags_after_inc(cpu, value);
        }
        // 16-bit register increments (don't need to Set Flags for these)
        // [0x03]
        AllRegisters::BC => {
            let new_bc = cpu.registers.get_bc().wrapping_add(1);
            cpu.registers.set_bc(new_bc);
            emu_cycles(cpu, 1);
        }
        // [0x13]
        AllRegisters::DE => {
            let new_de = cpu.registers.get_de().wrapping_add(1);
            cpu.registers.set_de(new_de);
            emu_cycles(cpu, 1);
        }
        // [0x23]
        AllRegisters::HL => {
            let new_hl = cpu.registers.get_hl().wrapping_add(1);
            cpu.registers.set_hl(new_hl);
            emu_cycles(cpu, 1);
        }
        // [0x33]
        AllRegisters::SP => {
            cpu.sp = cpu.sp.wrapping_add(1);
            emu_cycles(cpu, 1);
        }
    }
}

// [0xC2, 0xC3, 0xCA, 0xD2, 0xDA, 0xE9]
pub fn op_jp(cpu: &mut CPU, target: JumpTest) -> bool {
    if matches!(target, JumpTest::HL) {
        // For JP HL (0xE9), jump to the address in HL
        cpu.pc = cpu.registers.get_hl();
        emu_cycles(cpu, 1); // cycle here?
        true // Jump occurred
    } else {
        // For JP nn (0xC3) or JP cc, nn
        let least_significant = cpu.bus.read_byte(None, cpu.pc + 1) as u16;
        let most_significant = cpu.bus.read_byte(None, cpu.pc + 2) as u16;
        let nn_address = (most_significant << 8) | least_significant;

        if match_jump(cpu, &target) {
            // Check condition (Always is true)
            cpu.pc = nn_address;
            emu_cycles(cpu, 1); // cycle here?
            true // Jump occurred
        } else {
            // Condition false for JP cc, nn. PC will be advanced by 3 in execute loop.
            false // Jump did NOT occur
        }
    }
}

// [0xC4, 0xCC, 0xCD, 0xD4, 0xDC]
pub fn op_call(cpu: &mut CPU, target: JumpTest) -> u16 {
    // Jump to addr in bus or increment pc

    // Get Bytes
    let least_significant = cpu.bus.read_byte(None, cpu.pc + 1) as u16;
    let most_significant = cpu.bus.read_byte(None, cpu.pc + 2) as u16;

    cpu.pc = cpu.pc.wrapping_add(3); // idk why but we need to do this

    // Perform Operation & Implicit Return
    goto_addr(
        cpu,
        (most_significant << 8) | least_significant,
        target,
        true,
    )
}

// [0x18, 0x20, 0x28, 0x30, 0x38]
pub fn op_jr(cpu: &mut CPU, target: JumpTest) -> u16 {
    let jump_distance = cpu.bus.read_byte(None, cpu.pc + 1) as i8;
    //println!("Jump Distance: {:02X}", jump_distance);
    goto_addr(
        cpu,
        cpu.pc.wrapping_add(jump_distance as u16),
        target,
        false,
    )
}

// [0xC1, 0xD1, 0xE1, 0xF1]
pub fn op_pop(cpu: &mut CPU, target: StackTarget) {
    // Pop Low and High Bytes
    let low: u16 = stack_pop(cpu) as u16;
    let high: u16 = stack_pop(cpu) as u16;

    // Combine Bytes
    let combined: u16 = (high << 8) | low;

    // Perform Operation
    match target {
        // [0xF1]
        StackTarget::AF => {
            cpu.registers.set_af(combined & 0xFFF0);
        }
        // [0xC1]
        StackTarget::BC => {
            cpu.registers.set_bc(combined);
        }
        // [0xD1]
        StackTarget::DE => {
            cpu.registers.set_de(combined);
        }
        // [0xE1]
        StackTarget::HL => {
            cpu.registers.set_hl(combined);
        }
    }
}

// [0xC5, 0xD5, 0xE5, 0xF5]
pub fn op_push(cpu: &mut CPU, target: StackTarget) {
    match target {
        // [0xF5]
        StackTarget::AF => {
            let high: u16 = (cpu.registers.get_af() >> 8) & 0xFF as u16;
            stack_push(cpu, high as u8, true);

            let low: u16 = cpu.registers.get_af() & 0xFF as u16;
            stack_push(cpu, low as u8, true);
        }
        // [0xC5]
        StackTarget::BC => {
            let high: u16 = (cpu.registers.get_bc() >> 8) & 0xFF as u16;
            stack_push(cpu, high as u8, true);
            let low: u16 = cpu.registers.get_bc() & 0xFF as u16;
            stack_push(cpu, low as u8, true);
        }
        // [0xD5]
        StackTarget::DE => {
            let high: u16 = (cpu.registers.get_de() >> 8) & 0xFF as u16;
            stack_push(cpu, high as u8, true);

            let low: u16 = cpu.registers.get_de() & 0xFF as u16;
            stack_push(cpu, low as u8, true);
        }
        // [0xE5]
        StackTarget::HL => {
            let high: u16 = (cpu.registers.get_hl() >> 8) & 0xFF as u16;
            stack_push(cpu, high as u8, true);

            let low: u16 = cpu.registers.get_hl() & 0xFF as u16;
            stack_push(cpu, low as u8, true);
        }
    }
    emu_cycles(cpu, 1);
}

// [0xC0, 0xD0, 0xD8, 0xC8, 0xC9]
pub fn op_ret(cpu: &mut CPU, target: JumpTest) -> bool {
    // Cycle if condition is not Always
    if !matches!(target, JumpTest::Always) {
        emu_cycles(cpu, 1);
    }

    let jump = match_jump(cpu, &target);

    if jump {
        let low: u16 = stack_pop(cpu) as u16;
        let high: u16 = stack_pop(cpu) as u16;

        let n: u16 = (high << 8) | low;
        cpu.pc = n;
        emu_cycles(cpu, 1);
        return true; // Return happened
    }
    // If we reach here, the condition was false, no return happened
    false // Return did not happen
}

// [0xD9]
pub fn op_reti(cpu: &mut CPU) {
    // Update Interrupt
    cpu.bus.interrupt_controller.set_master_enabled(true);

    // Call RET Logic w Always so it executes, op_ret will handle PC
    op_ret(cpu, JumpTest::Always);
}

// [0xC7, 0xD7, 0xE7, 0xF7, 0xFC, 0xFD, 0xFE, 0xFF]
pub fn op_rst(cpu: &mut CPU, target: RestTarget) {
    let low: u16 = match target {
        RestTarget::Zero => 0x00,
        RestTarget::One => 0x08,
        RestTarget::Two => 0x10,
        RestTarget::Three => 0x18,
        RestTarget::Four => 0x20,
        RestTarget::Five => 0x28,
        RestTarget::Six => 0x30,
        RestTarget::Seven => 0x38,
    };

    // Advance PC to point to the instruction AFTER the 1-byte RST instruction.
    // This is the address that should be pushed onto the stack for a standard RST.
    cpu.pc = cpu.pc.wrapping_add(1);

    // Now call goto_addr. It will push the current (advanced) cpu.pc
    // and then set cpu.pc to the RST vector address (0x00XX).
    cpu.pc = goto_addr(cpu, 0x0000 | low, JumpTest::Always, true);
}
