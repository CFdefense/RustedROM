// Debug Module - Serial Communication and Timer Diagnostic System
//
// This module provides comprehensive debugging capabilities for the Game Boy emulator,
// including serial communication capture and timer system diagnostics.
//
// Serial Communication Debug:
// Captures serial communication output from Game Boy programs, particularly useful
// for running test ROMs and diagnostic programs that communicate results through
// the serial port.
//
// Serial Communication Protocol:
// The Game Boy serial system uses two registers:
// - 0xFF01 (SB): Serial transfer data register
// - 0xFF02 (SC): Serial transfer control register
//
// Debug Operation:
// When a program writes 0x81 to the control register (indicating transfer start
// with internal clock), this module captures the data byte from 0xFF01 and
// accumulates it in a thread-safe buffer for later output.
//
// Common Use Cases:
// - Blargg's test ROMs output test results via serial
// - Homebrew programs can use serial for debug logging
// - Diagnostic tools communicate status and error information
//
// Timer System Diagnostics:
// Provides comprehensive logging capabilities for the Game Boy's timer system,
// enabling detailed analysis of timer behavior, interrupt generation, and timing
// accuracy. Essential for debugging timer-related issues and verifying cycle-accurate
// timer implementation.
//
// Timer Logging Features:
// - Complete timer state snapshots (DIV, TIMA, TMA, TAC registers)
// - Interrupt flag monitoring (both raw and masked values)
// - CPU context information (PC, cycle count)
// - Timestamped entries with custom event messages
//
// Thread Safety:
// The debug message buffer uses Mutex synchronization to allow safe access
// from multiple threads in the emulator system.

use crate::hdw::bus::BUS;
use crate::hdw::cpu::CPU;
use crate::hdw::emu::EmuContext;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    /// Global debug message buffer protected by mutex for thread-safe access.
    ///
    /// Capacity of 1024 bytes should handle most debug output scenarios.
    /// Used to accumulate serial communication data from Game Boy programs.
    static ref DBG_MSG: Mutex<Vec<u8>> = Mutex::new(Vec::with_capacity(1024));
}

/// Updates debug system by checking for serial transfer requests.
///
/// Monitors the serial control register (0xFF02) for transfer requests (0x81)
/// and captures data from the serial data register (0xFF01) when detected.
/// Automatically resets the control register after capturing data.
///
/// # Arguments
///
/// * `bus` - Mutable reference to system bus for register access
///
/// # Serial Protocol
///
/// The value 0x81 in SC (0xFF02) indicates:
/// - Bit 7: Transfer start flag
/// - Bit 0: Internal clock selected
pub fn dbg_update(bus: &mut BUS) {
    if bus.read_byte(None, 0xFF02) == 0x81 {
        let c = bus.read_byte(None, 0xFF01);

        if let Ok(mut msg) = DBG_MSG.lock() {
            msg.push(c);
        } else {
            println!("Failed to lock DBG_MSG for updating");
        }

        bus.write_byte(0xFF02, 0);
    }
}

/// Outputs accumulated debug messages to console.
///
/// Prints all messages currently stored in the debug buffer.
/// Handles both valid UTF-8 strings and raw byte sequences.
/// Messages are output with "DBG:" prefix for easy identification.
///
/// # Arguments
///
/// None
///
/// # Output Format
///
/// - Valid UTF-8: `DBG: <message>`
/// - Invalid UTF-8: `DBG (non-UTF8): XX XX XX ...` (hex bytes)
///
/// # Thread Safety
///
/// Safely locks the debug message buffer. Prints error message if lock fails.
pub fn dbg_print() {
    if let Ok(msg) = DBG_MSG.lock() {
        if !msg.is_empty() {
            match std::str::from_utf8(&msg) {
                Ok(s) => {
                    println!();
                    print!("DBG: {}", s);
                }
                Err(_) => {
                    print!("DBG (non-UTF8): ");
                    for &byte in msg.iter() {
                        print!("{:02X} ", byte);
                    }
                    println!();
                }
            }
        }
    } else {
        println!("Failed to lock DBG_MSG for printing");
    }
}

/// Logs complete timer system state with context information.
///
/// Captures a comprehensive snapshot of the timer system including all registers,
/// interrupt states, and CPU context. Only logs when debug mode is active to
/// prevent performance impact during normal emulation.
///
/// # Arguments
///
/// * `cpu` - Reference to CPU for register and interrupt state access
/// * `ctx` - Shared emulator context containing timer state and cycle count
/// * `message` - Custom message describing the timer event or condition
///
/// # Output Format
///
/// TIMER_DEBUG - TICKS:12345678 DIV:ABCD TIMA:12 TMA:34 TAC:07 INT_FLAGS(raw):01 INT_FLAGS(masked):E1 IE_REG:0F IME:true PC:1234 - Custom message
///
/// # Log File
///
/// Logs are written to `logs/timer_debug.txt` in append mode.
/// Creates the logs directory if it doesn't exist.
///
/// # Debug Control
///
/// Logging only occurs when debug mode is enabled via `is_debug_enabled()`.
pub fn log_timer_state(cpu: &CPU, ctx: &Arc<Mutex<EmuContext>>, message: &str) {
    if !crate::hdw::emu::is_debug_enabled() {
        return;
    }

    let raw_int_flags = cpu.bus.interrupt_controller.get_int_flags();
    let masked_int_flags = cpu.bus.interrupt_controller.get_int_flags() | 0xE0;
    let (ticks, timer_div, timer_tima, timer_tma, timer_tac) = {
        let emu_ctx_locked = ctx.lock().unwrap();
        (
            emu_ctx_locked.ticks,
            emu_ctx_locked.timer.div,
            emu_ctx_locked.timer.tima,
            emu_ctx_locked.timer.tma,
            emu_ctx_locked.timer.tac,
        )
    };

    let log_entry = format!(
        "TIMER_DEBUG - TICKS:{:08X} DIV:{:04X} TIMA:{:02X} TMA:{:02X} TAC:{:02X} INT_FLAGS(raw):{:02X} INT_FLAGS(masked):{:02X} IE_REG:{:02X} IME:{} PC:{:04X} - {}\n",
        ticks,
        timer_div,
        timer_tima,
        timer_tma,
        timer_tac,
        raw_int_flags,
        masked_int_flags,
        cpu.bus.interrupt_controller.get_ie_register(),
        cpu.bus.interrupt_controller.is_master_enabled(),
        cpu.pc,
        message
    );

    if let Err(_) = std::fs::create_dir_all("logs") {
        return;
    }

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/timer_debug.txt")
    {
        let _ = file.write_all(log_entry.as_bytes());
    }
}
