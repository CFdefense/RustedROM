use crate::hdw::cpu::CPU;
use crate::hdw::emu::EmuContext;
/**
 * Debug Timer Module - Timer System Diagnostic Logging
 *
 * This module provides comprehensive logging capabilities for the Game Boy's timer system,
 * enabling detailed analysis of timer behavior, interrupt generation, and timing accuracy.
 * The logging system is essential for debugging timer-related issues and verifying
 * cycle-accurate timer implementation.
 *
 * Logging Features:
 * - Complete timer state snapshots (DIV, TIMA, TMA, TAC registers)
 * - Interrupt flag monitoring (both raw and masked values)
 * - CPU context information (PC, cycle count)
 * - Timestamped entries with custom event messages
 *
 * Debug Output Format:
 * Each log entry includes:
 * - TICKS: Current emulator cycle count (8-digit hex)
 * - DIV: 16-bit internal timer counter (4-digit hex)  
 * - TIMA: Timer counter value (2-digit hex)
 * - TMA: Timer modulo value (2-digit hex)
 * - TAC: Timer control register (2-digit hex)
 * - INT_FLAGS: Interrupt flags raw and masked (2-digit hex each)
 * - IE_REG: Interrupt enable register (2-digit hex)
 * - IME: Interrupt master enable flag (boolean)
 * - PC: Program counter (4-digit hex)
 * - Custom message describing the timer event
 *
 * File Output:
 * Logs are written to "logs/timer_debug.txt" with automatic directory creation.
 * Each entry is appended to allow continuous monitoring across emulator sessions.
 *
 * Debug Control:
 * Logging only occurs when debug mode is enabled, preventing performance impact
 * during normal emulation while providing detailed diagnostics when needed.
 */
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

/**
 * Logs complete timer system state with context information
 *
 * Captures a comprehensive snapshot of the timer system including all registers,
 * interrupt states, and CPU context. Only logs when debug mode is active.
 *
 * Arguments:
 * - cpu: Reference to CPU for register and interrupt state access
 * - ctx: Shared emulator context containing timer state and cycle count
 * - message: Custom message describing the timer event or condition
 *
 * Output Format:
 * TIMER_DEBUG - TICKS:12345678 DIV:ABCD TIMA:12 TMA:34 TAC:07 INT_FLAGS(raw):01 INT_FLAGS(masked):E1 IE_REG:0F IME:true PC:1234 - Custom message
 */
pub fn log_timer_state(cpu: &CPU, ctx: &Arc<Mutex<EmuContext>>, message: &str) {
    // Only log if debug mode is enabled
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

    // Create logs directory if it doesn't exist
    if let Err(_) = std::fs::create_dir_all("logs") {
        return; // If we can't create the directory, skip logging
    }

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/timer_debug.txt")
    {
        let _ = file.write_all(log_entry.as_bytes());
    }
}
