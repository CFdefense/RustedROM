// Game Boy CPU Stack Operations Module
//
// This module implements stack operations for the Game Boy CPU including
// push/pop operations for bytes and 16-bit words. Provides proper stack pointer
// management and cycle-accurate timing for stack-based operations.
//
// Stack Architecture:
// - Descending stack (grows downward from high to low addresses)
// - Stack pointer (SP) points to next available stack location
// - Initial SP value: 0xFFFE (top of high RAM)
// - Stack operations use Work RAM and High RAM regions
//
// Stack Operations:
// Push Operation (stack_push):
//   1. Decrement stack pointer (SP--)
//   2. Optionally consume 1 M-cycle for timing accuracy
//   3. Write value to memory at new SP location
//
// 16-bit Push Operation (stack_push16):
//   1. Push high byte of 16-bit value first
//   2. Push low byte of 16-bit value second
//   3. Maintains little-endian byte order on stack
//
// Pop Operation (stack_pop):
//   1. Read value from memory at current SP location
//   2. Increment stack pointer (SP++)
//   3. Consume 1 M-cycle for timing accuracy
//
// Memory Access:
// - Stack operations use standard bus interface
// - Stack memory located in Work RAM (0xC000-0xDFFF) and High RAM (0xFF80-0xFFFE)
// - No special stack memory protection or overflow detection
// - Stack can grow into any writable memory region
//
// Timing Behavior:
// - Optional cycle consumption for push operations (controlled by cycle parameter)
// - Automatic cycle consumption for pop operations
// - Timing matches original Game Boy stack operation timing
//
// Use Cases:
// - Function call/return mechanisms (CALL/RET instructions)
// - Interrupt handling (automatic register preservation)
// - Temporary value storage during complex operations
// - Subroutine parameter passing and local variables

use crate::hdw::cpu::CPU;
use crate::hdw::emu::emu_cycles;

/// Pushes an 8-bit value onto the stack.
///
/// Decrements the stack pointer and writes the value to the new stack location.
/// Optionally consumes one M-cycle for timing accuracy.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU
/// * `value` - The 8-bit value to push onto the stack
/// * `cycle` - Whether to consume an M-cycle for timing (true = consume cycle)
///
/// # Stack Behavior
///
/// 1. SP is decremented (SP--)
/// 2. If cycle is true, consumes 1 M-cycle
/// 3. Value is written to memory at new SP location
///
/// # Memory Layout
///
/// Stack grows downward from 0xFFFE. After push, SP points to the newly written value.
pub fn stack_push(cpu: &mut CPU, value: u8, cycle: bool) {
    // Decrement Stack Pointer
    cpu.sp -= 1;

    if cycle {
        emu_cycles(cpu, 1);
    }

    cpu.bus.write_byte(cpu.sp, value);
}

/// Pushes a 16-bit value onto the stack.
///
/// Pushes a 16-bit word onto the stack by pushing the high byte first,
/// then the low byte. This maintains proper little-endian byte order on the stack.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU
/// * `value` - The 16-bit value to push onto the stack
/// * `cycle` - Whether to consume M-cycles for timing (passed to stack_push)
///
/// # Stack Behavior
///
/// 1. High byte (bits 15-8) is pushed first
/// 2. Low byte (bits 7-0) is pushed second
/// 3. SP is decremented twice (once per byte)
/// 4. Resulting stack layout: [SP] = low byte, [SP+1] = high byte
///
/// # Examples
///
/// Pushing 0x1234:
/// - First push: 0x12 (high byte)
/// - Second push: 0x34 (low byte)
/// - Stack: [SP] = 0x34, [SP+1] = 0x12
pub fn stack_push16(cpu: &mut CPU, value: u16, cycle: bool) {
    // Push high byte
    stack_push(cpu, (value >> 8) as u8, cycle);
    // Push low byte
    stack_push(cpu, (value & 0xFF) as u8, cycle);
}

/// Pops an 8-bit value from the stack.
///
/// Reads a value from the current stack location, increments the stack pointer,
/// and consumes one M-cycle for timing accuracy.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU
///
/// # Returns
///
/// The 8-bit value popped from the stack.
///
/// # Stack Behavior
///
/// 1. Value is read from memory at current SP location
/// 2. SP is incremented (SP++)
/// 3. One M-cycle is consumed for timing
///
/// # Safety
///
/// Uses unsafe pointer operations to create a temporary mutable reference for
/// the read operation. This is safe because:
/// - The CPU reference is valid for the duration of the scope
/// - No other mutable references exist during this time
/// - The reference is only used for the read operation
pub fn stack_pop(cpu: &mut CPU) -> u8 {
    // Grab Original Address
    let address = cpu.sp;

    // Increment SP
    cpu.sp += 1;

    emu_cycles(cpu, 1);

    // Create a temporary mutable reference for the write operation
    {
        let cpu_ref = cpu as *mut CPU;
        // SAFETY: We're only creating a temporary reference and not modifying any state
        // The CPU reference is valid for the duration of this scope
        // We ensure no other mutable references exist during this time
        cpu.bus.read_byte(Some(unsafe { &mut *cpu_ref }), address)
    }
}
