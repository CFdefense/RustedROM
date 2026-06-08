// hdw/interrupts.rs
// Game Boy interrupt controller and hardware interrupt management
//
// The interrupts module implements the Game Boy's interrupt system with proper priority
// handling, timing, and coordination between hardware components. Manages interrupt
// enable/disable states and provides accurate interrupt behavior emulation.
//
// # Interrupt Priority System
//
// Interrupts are processed in fixed priority order (highest to lowest):
// 1. VBLANK - V-Blank interrupt (priority 1)
// 2. LCDSTAT - LCD Status interrupt (priority 2)
// 3. TIMER - Timer overflow interrupt (priority 3)
// 4. SERIAL - Serial transfer completion interrupt (priority 4)
// 5. JOYPAD - Button press interrupt (priority 5)
//
// Only one interrupt is processed per cycle. Higher priority interrupts preempt
// lower priority processing. The master enable flag (IME) globally controls all
// interrupt processing.
//
// # Interrupt Vector Table
//
// - 0x40: V-Blank Interrupt Vector - End of frame rendering
// - 0x48: LCD Status Interrupt Vector - PPU status changes
// - 0x50: Timer Interrupt Vector - Timer overflow
// - 0x58: Serial Interrupt Vector - Serial transfer completion
// - 0x60: Joypad Interrupt Vector - Button press events
//
// # Timing Accuracy
//
// - EI instruction enables interrupts after the next instruction (delayed enable)
// - DI instruction immediately disables interrupts
// - Interrupt handling automatically disables IME until RETI instruction
// - Proper stack manipulation during interrupt entry/exit
//
// # Hardware Integration
//
// - PPU generates VBLANK and LCDSTAT interrupts based on display timing
// - Timer generates TIMER interrupts on overflow conditions
// - Gamepad generates JOYPAD interrupts on button state changes
// - Serial controller generates SERIAL interrupts on transfer completion

use crate::hdw::cpu::CPU;
use crate::hdw::debug::log_timer_state;
use crate::hdw::emu::EmuContext;
use crate::hdw::stack::*;
use std::sync::Arc;
use std::sync::Mutex;

/// Game Boy interrupt types with their corresponding bit flags.
///
/// Each interrupt type has a unique bit position used in both the Interrupt Enable (IE)
/// register at 0xFFFF and the Interrupt Flags (IF) register at 0xFF0F. When an interrupt
/// is requested, its corresponding bit is set in IF. If the same bit is also set in IE
/// and the Interrupt Master Enable (IME) flag is set, the interrupt will be serviced.
///
/// # Priority Order
///
/// Interrupts are checked and serviced in the order listed below (highest to lowest priority):
/// 1. VBLANK (bit 0) - Highest priority
/// 2. LCDSTAT (bit 1)
/// 3. TIMER (bit 2)
/// 4. SERIAL (bit 3)
/// 5. JOYPAD (bit 4) - Lowest priority
#[derive(Copy, Clone)]
pub enum Interrupts {
    /// V-Blank interrupt (bit 0, priority 1).
    ///
    /// Triggered at the end of frame rendering when the PPU enters V-Blank period.
    /// Vector address: 0x40
    VBLANK = 1,

    /// LCD Status interrupt (bit 1, priority 2).
    ///
    /// Triggered by PPU status changes such as mode transitions or LYC=LY coincidence.
    /// Vector address: 0x48
    LCDSTAT = 2,

    /// Timer overflow interrupt (bit 2, priority 3).
    ///
    /// Triggered when the timer counter (TIMA) overflows from 0xFF to 0x00.
    /// Vector address: 0x50
    TIMER = 4,

    /// Serial transfer completion interrupt (bit 3, priority 4).
    ///
    /// Triggered when a serial data transfer completes.
    /// Vector address: 0x58
    SERIAL = 8,

    /// Joypad button press interrupt (bit 4, priority 5).
    ///
    /// Triggered when a button is pressed on the joypad.
    /// Vector address: 0x60
    JOYPAD = 16,
}

/// Game Boy interrupt controller managing interrupt enable/disable states and flags.
///
/// The InterruptController manages the Game Boy's interrupt system, including the
/// Interrupt Enable (IE) register, Interrupt Flags (IF) register, and the Interrupt
/// Master Enable (IME) flag. It handles interrupt requests, priority checking, and
/// the delayed enable behavior of the EI instruction.
///
/// # Registers
///
/// - IE Register (0xFFFF): Controls which interrupt types can trigger
/// - IF Register (0xFF0F): Holds pending interrupt flags
/// - IME Flag: Global interrupt enable/disable state
///
/// # Timing Behavior
///
/// The EI instruction has a delayed enable behavior - interrupts are enabled after
/// the instruction following EI executes. This is tracked by the `enabling_ime` flag.
#[derive(Default)]
pub struct InterruptController {
    /// Interrupt Enable register (0xFFFF).
    ///
    /// Each bit corresponds to an interrupt type. When set, that interrupt type
    /// can trigger if its corresponding IF bit is also set and IME is enabled.
    pub ie_register: u8,

    /// Interrupt Flags register (0xFF0F).
    ///
    /// Each bit indicates a pending interrupt request. Set by hardware when an
    /// interrupt condition occurs, cleared when the interrupt is serviced.
    pub int_flags: u8,

    /// Interrupt Master Enable (IME) flag.
    ///
    /// Global interrupt enable/disable state. When false, no interrupts can trigger
    /// regardless of IE and IF register values. Set by EI/RETI, cleared by DI/interrupt handling.
    pub master_enabled: bool,

    /// Delayed IME enable flag for EI instruction.
    ///
    /// The EI instruction enables interrupts after the next instruction executes.
    /// This flag tracks that delayed enable state.
    pub enabling_ime: bool,
}

impl InterruptController {
    /// Creates a new interrupt controller with all interrupts disabled.
    ///
    /// Initializes the controller with IE and IF registers cleared, IME disabled,
    /// and no delayed enable pending. This matches the Game Boy's power-on state.
    ///
    /// # Returns
    ///
    /// A new InterruptController with default disabled state
    pub fn new() -> Self {
        InterruptController {
            ie_register: 0,
            int_flags: 0,
            master_enabled: false,
            enabling_ime: false,
        }
    }

    /// Reads the Interrupt Enable (IE) register value.
    ///
    /// Returns the current IE register mask indicating which interrupt types
    /// are enabled to trigger.
    ///
    /// # Returns
    ///
    /// Current IE register value (0xFFFF)
    pub fn get_ie_register(&self) -> u8 {
        self.ie_register
    }

    /// Writes a value to the Interrupt Enable (IE) register.
    ///
    /// Sets which interrupt types are enabled to trigger. Each bit corresponds
    /// to an interrupt type from the Interrupts enum.
    ///
    /// # Arguments
    ///
    /// * `value` - New IE register value to set
    pub fn set_ie_register(&mut self, value: u8) {
        self.ie_register = value;
    }

    /// Reads the Interrupt Flags (IF) register value.
    ///
    /// Returns the current IF register showing which interrupts are pending.
    ///
    /// # Returns
    ///
    /// Current IF register value (0xFF0F)
    pub fn get_int_flags(&self) -> u8 {
        self.int_flags
    }

    /// Writes a value to the Interrupt Flags (IF) register.
    ///
    /// Sets which interrupts are pending. Software can write to this register
    /// to acknowledge or manually trigger interrupts.
    ///
    /// # Arguments
    ///
    /// * `value` - New IF register value to set
    pub fn set_int_flags(&mut self, value: u8) {
        self.int_flags = value;
    }

    /// Requests an interrupt by setting its flag in the IF register.
    ///
    /// Sets the corresponding bit in the Interrupt Flags register to indicate
    /// a pending interrupt. The interrupt will trigger if its IE bit is set
    /// and IME is enabled.
    ///
    /// # Arguments
    ///
    /// * `interrupt` - The interrupt type to request
    pub fn request_interrupt(&mut self, interrupt: Interrupts) {
        self.int_flags |= interrupt as u8;
    }

    /// Processes delayed IME enable from EI instruction.
    ///
    /// The EI instruction enables interrupts after the next instruction executes.
    /// This method checks if a delayed enable is pending and activates IME if so.
    /// Should be called after each instruction execution.
    ///
    /// # Returns
    ///
    /// true if IME was enabled this step, false otherwise
    pub fn step_ime(&mut self) -> bool {
        if self.enabling_ime {
            self.master_enabled = true;
            self.enabling_ime = false;
            true
        } else {
            false
        }
    }

    /// Checks if the Interrupt Master Enable (IME) flag is set.
    ///
    /// Returns the current state of the global interrupt enable flag.
    ///
    /// # Returns
    ///
    /// true if interrupts are globally enabled, false if disabled
    pub fn is_master_enabled(&self) -> bool {
        self.master_enabled
    }

    /// Sets the Interrupt Master Enable (IME) flag directly.
    ///
    /// Immediately enables or disables global interrupt processing. Used by
    /// DI instruction (disable) and interrupt handling (auto-disable).
    ///
    /// # Arguments
    ///
    /// * `value` - true to enable interrupts, false to disable
    pub fn set_master_enabled(&mut self, value: bool) {
        self.master_enabled = value;
    }

    /// Sets the delayed IME enable flag for EI instruction.
    ///
    /// Configures whether IME should be enabled after the next instruction.
    /// Used by the EI instruction to implement its delayed enable behavior.
    ///
    /// # Arguments
    ///
    /// * `value` - true to schedule delayed enable, false to cancel
    pub fn set_enabling_ime(&mut self, value: bool) {
        self.enabling_ime = value;
    }
}

/// Handles an interrupt by pushing PC to stack and jumping to interrupt vector.
///
/// Executes the interrupt handling sequence:
/// 1. Pushes the current program counter to the stack
/// 2. Sets PC to the interrupt vector address
///
/// The IME flag is cleared by the caller before this function is invoked.
/// The interrupted code will resume when RETI is executed.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU state
/// * `address` - Interrupt vector address to jump to (0x40, 0x48, 0x50, 0x58, or 0x60)
pub fn int_handle(cpu: &mut CPU, address: u16) {
    stack_push16(cpu, cpu.pc, false);
    cpu.pc = address;
}

/// Checks if a specific interrupt should trigger and handles it if so.
///
/// Tests whether the specified interrupt type is both requested (IF bit set)
/// and enabled (IE bit set). If both conditions are met, executes the interrupt
/// by calling int_handle, clears the IF bit, disables IME, and wakes the CPU
/// from halt state.
///
/// For timer interrupts, logs the timer state for debugging purposes.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU state
/// * `int_controller` - Mutable reference to the interrupt controller
/// * `ctx` - Shared emulation context for debug logging
/// * `address` - Interrupt vector address to jump to if interrupt triggers
/// * `int_type` - The interrupt type to check
///
/// # Returns
///
/// true if the interrupt was handled, false if not triggered
pub fn int_check(
    cpu: &mut CPU,
    int_controller: &mut InterruptController,
    ctx: &Arc<Mutex<EmuContext>>,
    address: u16,
    int_type: Interrupts,
) -> bool {
    if (int_controller.get_int_flags() & int_type as u8) != 0
        && (int_controller.ie_register & int_type as u8) != 0
    {
        if let Interrupts::TIMER = int_type {
            log_timer_state(cpu, ctx, "Timer interrupt triggered");
        }
        int_handle(cpu, address);
        int_controller.set_int_flags(int_controller.get_int_flags() & !(int_type as u8));
        int_controller.master_enabled = false;
        cpu.is_halted = false;
        return true;
    }
    false
}

/// Main interrupt processing function that checks all interrupts in priority order.
///
/// Checks each interrupt type in priority order (VBLANK highest to JOYPAD lowest)
/// and handles the first one that is both requested and enabled. Only one interrupt
/// is processed per call, ensuring proper priority handling.
///
/// The interrupt priority order is:
/// 1. VBLANK (0x40) - Highest priority
/// 2. LCDSTAT (0x48)
/// 3. TIMER (0x50)
/// 4. SERIAL (0x58)
/// 5. JOYPAD (0x60) - Lowest priority
///
/// This function should be called during CPU execution when IME is enabled.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU state
/// * `int_controller` - Mutable reference to the interrupt controller
/// * `ctx` - Shared emulation context for debug logging
pub fn cpu_handle_interrupts(
    cpu: &mut CPU,
    int_controller: &mut InterruptController,
    ctx: &Arc<Mutex<EmuContext>>,
) {
    if int_check(cpu, int_controller, ctx, 0x40, Interrupts::VBLANK) {
    } else if int_check(cpu, int_controller, ctx, 0x48, Interrupts::LCDSTAT) {
    } else if int_check(cpu, int_controller, ctx, 0x50, Interrupts::TIMER) {
    } else if int_check(cpu, int_controller, ctx, 0x58, Interrupts::SERIAL) {
    } else if int_check(cpu, int_controller, ctx, 0x60, Interrupts::JOYPAD) {
    }
}
