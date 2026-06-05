/*
  hdw/io.rs
  Info: I/O register interface for Game Boy hardware components
  Description: The io module implements memory-mapped I/O register access for all Game Boy hardware.
              Provides centralized register read/write functionality with proper component routing
              and debug capabilities for development and testing.

  I/O Register Map:
    FF00: Joypad Register - Input controller for D-pad and button states
    FF01-FF02: Serial Data - Serial communication transfer buffer and control
    FF04-FF07: Timer Registers - Programmable timer with divider and control
    FF0F: Interrupt Flags - Pending interrupt status flags
    FF10-FF3F: Audio Registers - 4-channel audio processing unit control
    FF40-FF4B: LCD Registers - Picture processing unit and display controller
    FF4C-FF7F: Unused Registers - Compatibility placeholder for unused addresses
    FFFF: Interrupt Enable - Global interrupt enable mask register

  Core Functions:
    io_read: Register Reader - Routes read requests to appropriate hardware components
    io_write: Register Writer - Routes write requests with proper side-effect handling

  Component Integration:
    - GamePad: Joypad input state and button matrix scanning
    - Timer: System timing, divider, and timer overflow interrupts
    - InterruptController: Hardware interrupt coordination and priority
    - PPU: Graphics rendering, LCD control, and video timing
    - AudioSystem: 4-channel sound synthesis and audio output
    - DMA: Direct memory access transfers for sprites and background

  Debug Features:
    - Conditional debug output for unimplemented registers
    - Timer state logging with context information
    - Serial communication monitoring
    - Register access tracing for development

  Threading Safety:
    - Thread-safe serial data access through Mutex protection
    - Global emulation context integration for timing coordination
    - Safe component state access during register operations
    - Deadlock prevention through proper lock ordering

  Hardware Compatibility:
    - Accurate register behavior matching original Game Boy
    - Proper side-effect handling for write-sensitive registers
    - Open bus behavior for unused register ranges
    - DMA transfer initiation through LCD register writes

  Error Handling:
    - Graceful handling of unimplemented register addresses
    - Safe fallback values for failed lock operations
    - Debug logging coordination with global debug state
    - Component failure isolation through return value checking
*/

// io.rs
use crate::hdw::apu::AudioSystem;
use crate::hdw::cpu::CPU;
use crate::hdw::debug_timer::log_timer_state;
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
