use crate::hdw::bus::BUS;
/**
 * Debug Module - Serial Communication Debug System
 *
 * This module implements a debug system that captures serial communication output
 * from Game Boy programs, particularly useful for running test ROMs and diagnostic
 * programs that communicate results through the serial port.
 *
 * Serial Communication Protocol:
 * The Game Boy serial system uses two registers:
 * - 0xFF01 (SB): Serial transfer data register
 * - 0xFF02 (SC): Serial transfer control register
 *
 * Debug Operation:
 * When a program writes 0x81 to the control register (indicating transfer start
 * with internal clock), this module captures the data byte from 0xFF01 and
 * accumulates it in a thread-safe buffer for later output.
 *
 * Common Use Cases:
 * - Blargg's test ROMs output test results via serial
 * - Homebrew programs can use serial for debug logging
 * - Diagnostic tools communicate status and error information
 *
 * Thread Safety:
 * The debug message buffer uses Mutex synchronization to allow safe access
 * from multiple threads in the emulator system.
 *
 * The module provides both continuous monitoring (dbg_update) and output
 * functions (dbg_print) for viewing accumulated debug messages.
 */
// Debug to retrieve data from serial from blaarg tests
use std::sync::Mutex;

// Thread-safe debug message buffer
lazy_static::lazy_static! {
    /// Global debug message buffer protected by mutex for thread-safe access
    /// Capacity of 1024 bytes should handle most debug output scenarios
    static ref DBG_MSG: Mutex<Vec<u8>> = Mutex::new(Vec::with_capacity(1024));
}

/**
 * Updates debug system by checking for serial transfer requests
 *
 * Monitors the serial control register (0xFF02) for transfer requests (0x81)
 * and captures data from the serial data register (0xFF01) when detected.
 *
 * Arguments:
 * - bus: Mutable reference to system bus for register access
 */
pub fn dbg_update(bus: &mut BUS) {
    if bus.read_byte(None, 0xFF02) == 0x81 {
        // Check for 0x81 to indicate transfer request with internal clock
        let c = bus.read_byte(None, 0xFF01); // get flag from serial

        if let Ok(mut msg) = DBG_MSG.lock() {
            msg.push(c); // add to debug vector
        } else {
            println!("Failed to lock DBG_MSG for updating");
        }

        bus.write_byte(0xFF02, 0); // reset flag
    }
}

/**
 * Outputs accumulated debug messages to console
 *
 * Prints all messages currently stored in the debug buffer.
 * Handles both valid UTF-8 strings and raw byte sequences.
 * Messages are output with "DBG:" prefix for easy identification.
 */
pub fn dbg_print() {
    if let Ok(msg) = DBG_MSG.lock() {
        if !msg.is_empty() {
            // parse vector
            // Convert bytes to string, handling invalid UTF-8
            match std::str::from_utf8(&msg) {
                Ok(s) => {
                    println!();
                    print!("DBG: {}", s);
                }
                Err(_) => {
                    // Fall back to printing individual bytes
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
