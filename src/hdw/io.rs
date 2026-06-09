// hdw/io.rs
// I/O register interface for Game Boy hardware components
//
// The io module implements memory-mapped I/O register access for all Game Boy hardware.
// Provides centralized register read/write functionality with proper component routing
// and debug capabilities for development and testing.
//
// # I/O Register Map
//
// - 0xFF00: Joypad Register - Input controller for D-pad and button states
// - 0xFF01-0xFF02: Serial Data - Serial communication transfer buffer and control
// - 0xFF04-0xFF07: Timer Registers - Programmable timer with divider and control
// - 0xFF0F: Interrupt Flags - Pending interrupt status flags
// - 0xFF10-0xFF3F: Audio Registers - 4-channel audio processing unit control
// - 0xFF40-0xFF4B: LCD Registers - Picture processing unit and display controller
// - 0xFF4C-0xFF7F: Unused Registers - Compatibility placeholder for unused addresses
// - 0xFFFF: Interrupt Enable - Global interrupt enable mask register (handled by bus)
//
// # Component Integration
//
// - GamePad: Joypad input state and button matrix scanning
// - Timer: System timing, divider, and timer overflow interrupts
// - InterruptController: Hardware interrupt coordination and priority
// - PPU: Graphics rendering, LCD control, and video timing
// - AudioSystem: 4-channel sound synthesis and audio output
// - DMA: Direct memory access transfers for sprites and background
//
// # Threading Safety
//
// - Thread-safe serial data access through Mutex protection
// - Global emulation context integration for timing coordination
// - Safe component state access during register operations
// - Deadlock prevention through proper lock ordering
//
// # Hardware Compatibility
//
// - Accurate register behavior matching original Game Boy
// - Proper side-effect handling for write-sensitive registers
// - Open bus behavior (0xFF) for unused register ranges
// - DMA transfer initiation through LCD register writes

use crate::hdw::apu::AudioSystem;
use crate::hdw::cpu::CPU;
use crate::hdw::debug::log_timer_state;
use crate::hdw::dma::DMA;
use crate::hdw::gamepad::GamePad;
use crate::hdw::interrupts::InterruptController;
use crate::hdw::ppu::PPU;
use std::sync::Mutex;

// Use the EMU_CONTEXT from the emu module
use crate::hdw::emu::EMU_CONTEXT;

// Thread-safe serial data using a Mutex
lazy_static::lazy_static! {
    static ref SERIAL_DATA: Mutex<[u8; 2]> = Mutex::new([0; 2]);
}

/// Reads a value from a memory-mapped I/O register.
///
/// Routes read requests to the appropriate hardware component based on the address.
/// Handles special cases like timer state logging and serial data access with proper
/// thread safety. Returns 0xFF for unused register ranges (open bus behavior).
///
/// # I/O Register Routing
///
/// - 0xFF00: Joypad input state
/// - 0xFF01-0xFF02: Serial communication data
/// - 0xFF04-0xFF07: Timer registers (DIV, TIMA, TMA, TAC)
/// - 0xFF0F: Interrupt flags register
/// - 0xFF10-0xFF3F: Audio registers (APU channels 1-4)
/// - 0xFF40-0xFF4B: LCD/PPU registers
/// - 0xFF4C-0xFF7F: Unused (returns 0xFF)
///
/// # Arguments
///
/// * `cpu` - Optional CPU reference for debug logging
/// * `address` - I/O register address to read from (0xFF00-0xFF7F range)
/// * `interrupt_controller` - Reference to interrupt controller for IF register
/// * `ppu` - Reference to PPU for LCD register reads
/// * `gamepad` - Reference to gamepad for joypad register
/// * `apu` - Reference to audio system for sound registers
///
/// # Returns
///
/// The value read from the specified I/O register, or 0xFF for unused addresses
///
/// # Thread Safety
///
/// Uses Mutex locks for serial data and global emulation context access.
/// Prints error messages if locks fail but continues execution safely.
pub fn io_read(
    cpu: Option<&CPU>,
    address: u16,
    interrupt_controller: &InterruptController,
    ppu: &PPU,
    gamepad: &GamePad,
    apu: &AudioSystem,
) -> u8 {
    let value = match address {
        0xFF00 => gamepad.get_gamepad_output(),
        0xFF01 => {
            if let Ok(data) = SERIAL_DATA.lock() {
                data[0]
            } else {
                println!("Failed to lock SERIAL_DATA for reading");
                0
            }
        }
        0xFF02 => {
            if let Ok(data) = SERIAL_DATA.lock() {
                data[1]
            } else {
                println!("Failed to lock SERIAL_DATA for reading");
                0
            }
        }
        0xFF04..=0xFF07 => {
            if let Some(ctx_arc) = EMU_CONTEXT.get() {
                if let Ok(emu_ctx_lock) = ctx_arc.lock() {
                    let val = emu_ctx_lock.timer.timer_read(address);
                    val
                } else {
                    eprintln!("io_read (timer): Failed to lock EmuContext");
                    0
                }
            } else {
                eprintln!("io_read (timer): Global EmuContext not initialized");
                0
            }
        }
        0xFF0F => {
            let val = interrupt_controller.get_int_flags();
            if let Some(c) = cpu {
                if let Some(ctx_arc) = EMU_CONTEXT.get() {
                    if crate::hdw::emu::is_debug_enabled() {
                        log_timer_state(
                            c,
                            ctx_arc,
                            &format!("Reading INT_FLAGS from FF0F = {:02X}", val),
                        );
                    }
                }
            }
            val
        }
        0xFF10..=0xFF3F => {
            // Sound registers
            apu.read_register(address)
        }
        0xFF40..=0xFF4B => ppu.lcd.lcd_read(address),
        0xFF4C..=0xFF7F => {
            // Unused I/O registers (including FF7F)
            // Some games write to these addresses, but they don't do anything
            // Return 0xFF for compatibility (open bus behavior)
            0xFF
        }
        _ => {
            if crate::hdw::emu::is_debug_enabled() {
                println!("IO READ NOT IMPLEMENTED for address: {:04X}", address);
            }
            0
        }
    };

    value
}

/// Writes a value to a memory-mapped I/O register.
///
/// Routes write requests to the appropriate hardware component based on the address.
/// Handles side effects like DMA transfer initiation and timer state changes. Silently
/// ignores writes to unused register ranges for compatibility.
///
/// # I/O Register Routing
///
/// - 0xFF00: Joypad button/direction selection
/// - 0xFF01-0xFF02: Serial communication data and control
/// - 0xFF04-0xFF07: Timer registers (DIV, TIMA, TMA, TAC)
/// - 0xFF0F: Interrupt flags register
/// - 0xFF10-0xFF3F: Audio registers (APU channels 1-4)
/// - 0xFF40-0xFF4B: LCD/PPU registers (may trigger DMA)
/// - 0xFF4C-0xFF7F: Unused (writes ignored)
///
/// # Side Effects
///
/// - Writing to 0xFF46 (DMA register via LCD) initiates DMA transfer
/// - Writing to timer registers may trigger interrupts
/// - Writing to LCD registers may affect PPU state
/// - Serial writes update transfer state
///
/// # Arguments
///
/// * `address` - I/O register address to write to (0xFF00-0xFF7F range)
/// * `value` - Value to write to the register
/// * `dma` - Mutable reference to DMA controller for transfer initiation
/// * `interrupt_controller` - Mutable reference to interrupt controller for IF register
/// * `ppu` - Mutable reference to PPU for LCD register writes
/// * `gamepad` - Mutable reference to gamepad for joypad selection
/// * `apu` - Mutable reference to audio system for sound registers
///
/// # Thread Safety
///
/// Uses Mutex locks for serial data and global emulation context access.
/// Prints error messages if locks fail but continues execution safely.
pub fn io_write(
    address: u16,
    value: u8,
    dma: &mut DMA,
    interrupt_controller: &mut InterruptController,
    ppu: &mut PPU,
    gamepad: &mut GamePad,
    apu: &mut AudioSystem,
) {
    match address {
        0xFF00 => {
            gamepad.gamepad_set_selection(value);
        }
        0xFF01 => {
            if let Ok(mut data) = SERIAL_DATA.lock() {
                data[0] = value;
                return;
            } else {
                println!("Failed to lock SERIAL_DATA for writing to SB");
            }
        }
        0xFF02 => {
            if let Ok(mut data) = SERIAL_DATA.lock() {
                data[1] = value;
                return;
            } else {
                println!("Failed to lock SERIAL_DATA for writing to SC");
            }
        }
        0xFF04..=0xFF07 => {
            if let Some(ctx_arc) = EMU_CONTEXT.get() {
                if let Ok(mut emu_ctx_lock) = ctx_arc.lock() {
                    // Store values we need for logging before modifying timer
                    if address == 0xFF07 {
                        emu_ctx_lock.timer.tac
                    } else {
                        0
                    };

                    // Do the actual timer write
                    emu_ctx_lock.timer.timer_write(address, value);

                    // Release the lock before logging
                    drop(emu_ctx_lock);
                }
            } else {
                eprintln!("io_write (timer): Global EmuContext not initialized");
            }
            return;
        }
        0xFF0F => {
            interrupt_controller.set_int_flags(value);
            return;
        }
        0xFF10..=0xFF3F => {
            // Sound registers
            apu.write_register(address, value);
        }
        0xFF40..=0xFF4B => {
            let result = ppu.lcd.lcd_write(address, value);

            // if lcd write returns a value we know to initiate a dma transfer
            if let Some(dma_value) = result {
                dma.dma_start(dma_value);

                if crate::hdw::emu::is_debug_enabled() {
                    println!("DMA STARTED");
                }
            }
        }
        0xFF4C..=0xFF7F => {
            // Unused I/O registers (including FF7F)
            // Some games write to these addresses, but they don't do anything
            // Just ignore the write silently for compatibility
        }
        _ => {
            if crate::hdw::emu::is_debug_enabled() {
                println!("IO WRITE NOT IMPLEMENTED for address: {:04X}", address);
            }
        }
    }
}
