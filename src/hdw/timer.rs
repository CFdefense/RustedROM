// Timer Module - Game Boy Hardware Timer System
//
// This module implements the Game Boy's timing system, which consists of a 16-bit internal
// counter (DIV) and a configurable timer (TIMA/TMA/TAC). The timer system is crucial for
// game timing, sound generation, and various game mechanics that depend on precise timing.
//
// Hardware Components:
// - DIV: 16-bit internal divider register (upper 8 bits readable at 0xFF04)
// - TIMA: 8-bit timer counter that increments based on TAC frequency setting
// - TMA: 8-bit timer modulo - value loaded into TIMA when it overflows
// - TAC: 8-bit timer control register (enable bit + 2-bit frequency select)
//
// Timer Frequencies (based on DIV bit transitions):
// - 00: 4096 Hz (bit 9 of internal counter)
// - 01: 262144 Hz (bit 3 of internal counter)
// - 10: 65536 Hz (bit 5 of internal counter)
// - 11: 16384 Hz (bit 7 of internal counter)
//
// The timer system generates interrupts when TIMA overflows from 0xFF to 0x00,
// at which point TIMA is reloaded with the TMA value and a timer interrupt is requested.
//
// Timing Accuracy:
// The implementation uses edge detection on specific bits of the internal counter
// to achieve cycle-accurate timer behavior that matches original Game Boy hardware.

use core::panic;

use crate::hdw::cpu::CPU;
use crate::hdw::interrupts::Interrupts;

/// Game Boy Timer Controller.
///
/// Manages the internal 16-bit counter and user-programmable timer system.
/// Handles timer overflow interrupts and provides accurate timing for games.
///
/// # Hardware Registers
///
/// - 0xFF04 (DIV): Divider register - upper 8 bits of internal 16-bit counter
/// - 0xFF05 (TIMA): Timer counter - increments at selected frequency
/// - 0xFF06 (TMA): Timer modulo - reload value for TIMA on overflow
/// - 0xFF07 (TAC): Timer control - enable bit and frequency selection
///
/// # Timer Operation
///
/// The timer uses edge detection on specific bits of the internal DIV counter
/// to generate TIMA increments at the selected frequency. When TIMA overflows
/// from 0xFF to 0x00, it is reloaded with TMA and a timer interrupt is requested.
pub struct Timer {
    /// 16-bit internal divider register - increments every CPU cycle
    /// Only upper 8 bits are exposed to software at address 0xFF04
    pub div: u16,

    /// 8-bit timer counter - increments at frequency determined by TAC
    /// Address 0xFF05 - generates interrupt when overflowing from 0xFF to 0x00
    pub tima: u8,

    /// 8-bit timer modulo - value loaded into TIMA after overflow
    /// Address 0xFF06 - allows games to set custom timer periods
    pub tma: u8,

    /// 8-bit timer control register - enables timer and sets frequency
    /// Address 0xFF07 - bit 2 enables timer, bits 0-1 select frequency
    pub tac: u8,
}

impl Timer {
    /// Creates a new Timer instance with hardware-accurate initial values.
    ///
    /// Initializes the timer with a common startup DIV value (0xAC00) that
    /// is often seen in hardware logs. Other registers start at 0.
    ///
    /// # Returns
    ///
    /// New Timer with DIV set to common startup value
    pub fn new() -> Self {
        Timer {
            // Initial value often seen in logs - represents startup state
            div: 0xAC00,
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }

    /// Advances timer by one CPU cycle and handles TIMA updates.
    ///
    /// This function implements the Game Boy's timer behavior using edge detection
    /// on specific bits of the internal DIV counter. When the selected bit transitions
    /// from 1 to 0, TIMA is incremented if the timer is enabled.
    ///
    /// # Timer Frequency Detection
    ///
    /// - 00 (4096 Hz): Detects bit 9 transition (1->0)
    /// - 01 (262144 Hz): Detects bit 3 transition (1->0)
    /// - 10 (65536 Hz): Detects bit 5 transition (1->0)
    /// - 11 (16384 Hz): Detects bit 7 transition (1->0)
    ///
    /// When TIMA overflows from 0xFF, it is reloaded with TMA and a timer
    /// interrupt is requested.
    ///
    /// # Arguments
    ///
    /// * `cpu` - Mutable reference to CPU for interrupt handling
    pub fn timer_tick(&mut self, cpu: &mut CPU) {
        let prev_div: u16 = self.div;
        self.div = self.div.wrapping_add(1);

        let tima_should_increment: bool;

        // Edge detection on DIV bits based on TAC frequency setting
        // Each frequency corresponds to a specific bit of the internal counter
        match self.tac & 0b11 {
            0b00 => {
                // 4096 Hz - bit 9 transition from 1->0
                tima_should_increment = (prev_div & (1 << 9)) != 0 && (self.div & (1 << 9)) == 0;
            }
            0b01 => {
                // 262144 Hz - bit 3 transition from 1->0
                tima_should_increment = (prev_div & (1 << 3)) != 0 && (self.div & (1 << 3)) == 0;
            }
            0b10 => {
                // 65536 Hz - bit 5 transition from 1->0
                tima_should_increment = (prev_div & (1 << 5)) != 0 && (self.div & (1 << 5)) == 0;
            }
            0b11 => {
                // 16384 Hz - bit 7 transition from 1->0
                tima_should_increment = (prev_div & (1 << 7)) != 0 && (self.div & (1 << 7)) == 0;
            }
            _ => unreachable!(),
        }

        // Only increment TIMA if timer is enabled (bit 2 of TAC) and should increment
        if tima_should_increment && (self.tac & (1 << 2)) != 0 {
            self.tima = self.tima.wrapping_add(1);

            // Check for overflow - when TIMA reaches 0xFF and wraps to 0x00
            if self.tima == 0xFF {
                // Reload TIMA with modulo value and request timer interrupt
                self.tima = self.tma;
                cpu.cpu_request_interrupt(Interrupts::TIMER);
            }
        }
    }

    /// Handles writes to timer registers with hardware-accurate behavior.
    ///
    /// # Arguments
    ///
    /// * `address` - Timer register address (0xFF04-0xFF07)
    /// * `value` - 8-bit value to write
    ///
    /// # Register Behavior
    ///
    /// - 0xFF04 (DIV): Writing any value resets the entire 16-bit internal counter to 0
    /// - 0xFF05 (TIMA): Sets the timer counter value
    /// - 0xFF06 (TMA): Sets the timer modulo (reload value)
    /// - 0xFF07 (TAC): Sets timer control (enable and frequency)
    ///
    /// # Panics
    ///
    /// Panics if an unsupported address is provided.
    pub fn timer_write(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 => {
                // Writing to DIV (0xFF04) resets the *entire* 16-bit internal counter
                // This is a critical behavior for timer accuracy
                self.div = 0;
            }
            0xFF05 => self.tima = value, // TIMA - Timer counter
            0xFF06 => self.tma = value,  // TMA - Timer modulo
            0xFF07 => self.tac = value,  // TAC - Timer control
            _ => panic!("UNSUPPORTED TIMER WRITE ADDRESS: {:#06X}", address),
        }
    }

    /// Handles reads from timer registers.
    ///
    /// # Arguments
    ///
    /// * `address` - Timer register address (0xFF04-0xFF07)
    ///
    /// # Returns
    ///
    /// The 8-bit register value at the specified address.
    ///
    /// # Register Behavior
    ///
    /// - 0xFF04 (DIV): Returns upper 8 bits of the 16-bit internal counter
    /// - 0xFF05 (TIMA): Returns current timer counter value
    /// - 0xFF06 (TMA): Returns timer modulo value
    /// - 0xFF07 (TAC): Returns timer control register
    ///
    /// # Panics
    ///
    /// Panics if an unsupported address is provided.
    pub fn timer_read(&self, address: u16) -> u8 {
        match address {
            0xFF04 => {
                // Reading DIV (0xFF04) returns the upper 8 bits of the 16-bit internal counter
                // This provides a continuously incrementing value visible to software
                (self.div >> 8) as u8
            }
            0xFF05 => self.tima, // TIMA - Timer counter
            0xFF06 => self.tma,  // TMA - Timer modulo
            0xFF07 => self.tac,  // TAC - Timer control
            _ => panic!("UNSUPPORTED TIMER READ ADDRESS: {:#06X}", address),
        }
    }
}
