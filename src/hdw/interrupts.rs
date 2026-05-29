/*
  hdw/interrupts.rs
  Info: Game Boy interrupt controller and hardware interrupt management
  Description: The interrupts module implements the Game Boy's interrupt system with proper priority
              handling, timing, and coordination between hardware components. Manages interrupt
              enable/disable states and provides accurate interrupt behavior emulation.

  Interrupts Enum:
    VBLANK: V-Blank Interrupt - Triggered at end of frame rendering (priority 1)
    LCDSTAT: LCD Status Interrupt - Triggered by PPU status changes (priority 2)
    TIMER: Timer Interrupt - Triggered by timer overflow (priority 3)
    SERIAL: Serial Interrupt - Triggered by serial transfer completion (priority 4)
    JOYPAD: Joypad Interrupt - Triggered by button press events (priority 5)

  InterruptController Struct Members:
    ie_register: Interrupt Enable Register - Controls which interrupts can trigger (FFFF)
    int_flags: Interrupt Flags Register - Pending interrupt status flags (FF0F)
    master_enabled: Interrupt Master Enable - Global interrupt enable/disable state (IME)
    enabling_ime: Delayed IME Enable - Flag for EI instruction's delayed enable behavior

  Core Functions:
    InterruptController::new: Constructor - Initializes interrupt controller with default disabled state
    get_ie_register: IE Register Reader - Returns interrupt enable mask register value
    set_ie_register: IE Register Writer - Sets interrupt enable mask register value
    get_int_flags: IF Register Reader - Returns pending interrupt flags register value
    set_int_flags: IF Register Writer - Sets interrupt flags register value
    request_interrupt: Interrupt Request - Sets interrupt flag for specific interrupt type
    step_ime: IME Delay Handler - Processes delayed interrupt master enable after EI instruction
    is_master_enabled: IME Status Query - Returns current interrupt master enable state
    set_master_enabled: IME Control - Directly sets interrupt master enable state
    set_enabling_ime: IME Delay Setup - Configures delayed IME enable for EI instruction

  Interrupt Processing Functions:
    int_handle: Interrupt Handler - Executes interrupt by pushing PC to stack and jumping to vector
    int_check: Interrupt Checker - Tests if specific interrupt should trigger and handles it
    cpu_handle_interrupts: Main Processor - Checks all interrupts in priority order

  Interrupt Vector Table:
    0x40: V-Blank Interrupt Vector - End of frame rendering interrupt
    0x48: LCD Status Interrupt Vector - PPU status change interrupt
    0x50: Timer Interrupt Vector - Timer overflow interrupt
    0x58: Serial Interrupt Vector - Serial transfer completion interrupt
    0x60: Joypad Interrupt Vector - Button press interrupt

  Priority System:
    - Interrupts processed in fixed priority order (VBLANK highest, JOYPAD lowest)
    - Only one interrupt processed per cycle
    - Higher priority interrupts preempt lower priority processing
    - Master enable flag globally controls all interrupt processing

  Timing Accuracy:
    - EI instruction enables interrupts after next instruction (delayed enable)
    - DI instruction immediately disables interrupts
    - Interrupt handling automatically disables IME until RETI instruction
    - Proper stack manipulation during interrupt entry/exit

  Hardware Integration:
    - PPU generates VBLANK and LCDSTAT interrupts based on display timing
    - Timer generates TIMER interrupts on overflow conditions
    - Gamepad generates JOYPAD interrupts on button state changes
    - Serial controller generates SERIAL interrupts on transfer completion

  Debug Integration:
    - Timer interrupt logging for debugging timer-related issues
    - Context integration for state inspection during interrupt processing
    - Interrupt request tracing for development and testing
    - Timing coordination with emulation context
*/

use crate::hdw::cpu::CPU;
use crate::hdw::debug_timer::log_timer_state;
use crate::hdw::emu::EmuContext;
use crate::hdw::stack::*;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Copy, Clone)]
pub enum Interrupts {
    VBLANK = 1,
    LCDSTAT = 2,
    TIMER = 4,
    SERIAL = 8,
    JOYPAD = 16,
}

#[derive(Default)]
pub struct InterruptController {
    pub ie_register: u8,      // Interrupt Enable register (0xFFFF)
    pub int_flags: u8,        // Interrupt Flags register (0xFF0F)
    pub master_enabled: bool, // IME (Interrupt Master Enable)
    pub enabling_ime: bool,   // Flag for delayed IME enabling after EI
}

impl InterruptController {
    pub fn new() -> Self {
        InterruptController {
            ie_register: 0,
            int_flags: 0,
            master_enabled: false,
            enabling_ime: false,
        }
    }

    pub fn get_ie_register(&self) -> u8 {
        self.ie_register
    }

    pub fn set_ie_register(&mut self, value: u8) {
        self.ie_register = value;
    }

    pub fn get_int_flags(&self) -> u8 {
        self.int_flags
    }

    pub fn set_int_flags(&mut self, value: u8) {
        self.int_flags = value;
    }

    pub fn request_interrupt(&mut self, interrupt: Interrupts) {
        self.int_flags |= interrupt as u8;
    }

    pub fn step_ime(&mut self) -> bool {
        if self.enabling_ime {
            self.master_enabled = true;
            self.enabling_ime = false;
            true
        } else {
            false
        }
    }

    pub fn is_master_enabled(&self) -> bool {
        self.master_enabled
    }

    pub fn set_master_enabled(&mut self, value: bool) {
        self.master_enabled = value;
    }

    pub fn set_enabling_ime(&mut self, value: bool) {
        self.enabling_ime = value;
    }
}

pub fn int_handle(cpu: &mut CPU, address: u16) {
    stack_push16(cpu, cpu.pc, false);
    cpu.pc = address;
}

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
