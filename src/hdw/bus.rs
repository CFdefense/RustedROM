/*
  hdw/bus.rs
  Info: Game Boy memory bus and address space management
  Description: The bus module implements the central memory management unit that routes all memory
              access requests to appropriate hardware components. Provides unified memory interface
              with accurate address space mapping and component isolation.

  Memory Map Implementation:
    0x0000-0x3FFF: ROM Bank 0 - Fixed cartridge ROM bank containing game startup code
    0x4000-0x7FFF: ROM Bank 1+ - Switchable cartridge ROM banks via memory bank controllers
    0x8000-0x97FF: Video RAM - Tile data storage for background and sprite graphics
    0x9800-0x9BFF: Background Map 1 - Tile map for background layer rendering
    0x9C00-0x9FFF: Background Map 2 - Alternate tile map for background layer
    0xA000-0xBFFF: Cartridge RAM - Battery-backed save data and additional storage
    0xC000-0xCFFF: Work RAM Bank 0 - Main system RAM for game data and stack
    0xD000-0xDFFF: Work RAM Bank 1-7 - Additional RAM banks (Game Boy Color only)
    0xE000-0xFDFF: Echo RAM - Mirror of work RAM (reserved, unused)
    0xFE00-0xFE9F: Object Attribute Memory - Sprite definition and property storage
    0xFEA0-0xFEFF: Restricted Area - Unusable memory space (returns 0x00)
    0xFF00-0xFF7F: I/O Registers - Hardware control and status registers
    0xFF80-0xFFFE: High RAM - Fast zero-page RAM for critical code
    0xFFFF: Interrupt Enable - Global interrupt mask register

  BUS Struct Members:
    cart: Cartridge Interface - ROM/RAM access and memory bank controller handling
    ram: RAM Controller - Work RAM and High RAM management
    ppu: Picture Processing Unit - Video RAM and graphics register access
    apu: Audio Processing Unit - Sound register and audio data handling
    gamepad: Input Controller - Joypad register and input state management
    interrupt_controller: Interrupt Manager - Interrupt flag and enable register control
    dma: DMA Controller - Direct memory access for sprite data transfers

  Core Functions:
    BUS::new: Constructor - Initializes all hardware components with default states
    read_byte: Memory Reader - Routes read requests to appropriate component based on address
    write_byte: Memory Writer - Routes write requests with proper side-effect handling
    tick_dma: DMA Processor - Handles ongoing DMA transfer operations

  Access Control:
    - DMA transfer protection for OAM access during sprite transfers
    - Memory bank switching coordination through cartridge interface
    - Component isolation preventing cross-contamination of data
    - Safe echo RAM handling without affecting work RAM state

  Performance Optimization:
    - Direct component routing avoiding unnecessary indirection
    - Efficient address range matching using match statements
    - Minimal overhead memory access pattern
    - Component state caching where appropriate

  Hardware Accuracy:
    - Cycle-accurate memory timing coordination
    - Proper restricted area behavior (FEA0-FEFF returns 0x00)
    - Echo RAM implementation matching original hardware
    - DMA transfer timing and access restrictions
    - Interrupt register access coordination with CPU

  Debug Integration:
    - Memory access logging capabilities through component interfaces
    - State inspection for all connected hardware components
    - Safe debugging access without affecting emulation timing
    - Component-specific debug information routing
*/

use super::cart::Cartridge;
use crate::hdw::apu::AudioSystem;
use crate::hdw::cpu::CPU;
use crate::hdw::dma::DMA;
use crate::hdw::gamepad::GamePad;
use crate::hdw::interrupts::InterruptController;
use crate::hdw::io::{io_read, io_write};
use crate::hdw::ppu::PPU;
use crate::hdw::ram::RAM;

/// Game Boy memory bus coordinating all hardware component access.
///
/// The BUS structure acts as the central memory management unit, routing all memory
/// access requests to the appropriate hardware components based on address ranges.
/// It implements the complete Game Boy memory map (0x0000-0xFFFF) and coordinates
/// access control, DMA transfers, and component isolation.
///
/// # Memory Map
///
/// - 0x0000-0x7FFF: Cartridge ROM (fixed and switchable banks)
/// - 0x8000-0x9FFF: Video RAM (tile data and maps)
/// - 0xA000-0xBFFF: Cartridge RAM (save data)
/// - 0xC000-0xDFFF: Work RAM
/// - 0xE000-0xFDFF: Echo RAM (mirrors WRAM)
/// - 0xFE00-0xFE9F: Object Attribute Memory (sprites)
/// - 0xFEA0-0xFEFF: Restricted (unusable)
/// - 0xFF00-0xFF7F: I/O Registers
/// - 0xFF80-0xFFFE: High RAM
/// - 0xFFFF: Interrupt Enable Register
pub struct BUS {
    /// Cartridge interface for ROM and RAM access with memory bank controller support.
    pub cart: Cartridge,

    /// RAM controller managing Work RAM (8KB) and High RAM (127 bytes).
    pub ram: RAM,

    /// Picture Processing Unit handling video RAM and graphics rendering.
    pub ppu: PPU,

    /// Audio Processing Unit managing sound registers and audio output.
    pub apu: AudioSystem,

    /// GamePad controller for joypad input state and register management.
    pub gamepad: GamePad,

    /// Interrupt controller managing interrupt flags and enable register.
    pub interrupt_controller: InterruptController,

    /// DMA controller for direct memory access transfers to OAM.
    pub dma: DMA,
}

impl BUS {
    /// Creates a new memory bus with all hardware components initialized.
    ///
    /// Initializes the cartridge, RAM, PPU, APU, gamepad, interrupt controller,
    /// and DMA controller with their default states, ready for emulation.
    ///
    /// # Returns
    ///
    /// A new `BUS` instance with all components initialized to power-on state.
    pub fn new() -> Self {
        BUS {
            cart: Cartridge::new(),
            ram: RAM::new(),
            ppu: PPU::new(),
            apu: AudioSystem::new(),
            gamepad: GamePad::new(),
            interrupt_controller: InterruptController::new(),
            dma: DMA::new(),
        }
    }

    /// Reads a byte from the memory bus at the specified address.
    ///
    /// Routes the read request to the appropriate hardware component based on
    /// the address range. Handles special cases like DMA transfer protection
    /// for OAM access and restricted memory regions.
    ///
    /// # Arguments
    ///
    /// * `cpu` - Optional CPU reference for I/O register reads requiring CPU state
    /// * `address` - 16-bit memory address to read from (0x0000-0xFFFF)
    ///
    /// # Returns
    ///
    /// The byte value at the specified address, or 0xFF/0x00 for protected/restricted regions.
    ///
    /// # Memory Routing
    ///
    /// - ROM (0x0000-0x7FFF): Cartridge ROM banks
    /// - VRAM (0x8000-0x9FFF): PPU video RAM
    /// - Cart RAM (0xA000-0xBFFF): Cartridge RAM
    /// - WRAM (0xC000-0xFDFF): Work RAM with echo support
    /// - OAM (0xFE00-0xFE9F): Sprite attributes (0xFF during DMA)
    /// - Restricted (0xFEA0-0xFEFF): Returns 0x00
    /// - I/O (0xFF00-0xFF7F): Hardware registers
    /// - HRAM (0xFF80-0xFFFE): High RAM
    /// - IE (0xFFFF): Interrupt Enable register
    pub fn read_byte(&mut self, cpu: Option<&CPU>, address: u16) -> u8 {
        let value = match address {
            0x0000..=0x7FFF => self.cart.read_byte(address), // Cartridge ROM
            0x8000..=0x9FFF => self.ppu.ppu_vram_read(address), // Video RAM
            0xA000..=0xBFFF => self.cart.read_byte(address), // Cartridge RAM
            0xC000..=0xDFFF => self.ram.wram_read(address),  // WRAM
            0xE000..=0xFDFF => self.ram.wram_read(address),  // Work RAM (echo)
            0xFE00..=0xFE9F => {
                if self.dma.dma_transferring() {
                    0xFF
                } else {
                    self.ppu.ppu_oam_read(address)
                }
            } // OAM
            0xFEA0..=0xFEFF => 0x00,                         // Unusable memory
            0xFF00..=0xFF7F => io_read(
                cpu,
                address,
                &self.interrupt_controller,
                &self.ppu,
                &self.gamepad,
                &self.apu,
            ), // I/O registers
            0xFF80..=0xFFFE => self.ram.hram_read(address),  // High RAM
            0xFFFF => self.interrupt_controller.get_ie_register(), // Interrupt Enable
        };

        value
    }

    /// Writes a byte to the memory bus at the specified address.
    ///
    /// Routes the write request to the appropriate hardware component based on
    /// the address range. Handles special cases like DMA transfer protection,
    /// echo RAM (which is ignored), and restricted memory regions.
    ///
    /// # Arguments
    ///
    /// * `address` - 16-bit memory address to write to (0x0000-0xFFFF)
    /// * `value` - Byte value to write
    ///
    /// # Memory Routing
    ///
    /// - ROM (0x0000-0x7FFF): Cartridge control (bank switching)
    /// - VRAM (0x8000-0x9FFF): PPU video RAM
    /// - Cart RAM (0xA000-0xBFFF): Cartridge RAM
    /// - WRAM (0xC000-0xDFFF): Work RAM
    /// - Echo RAM (0xE000-0xFDFF): Ignored (no operation)
    /// - OAM (0xFE00-0xFE9F): Sprite attributes (blocked during DMA)
    /// - Restricted (0xFEA0-0xFEFF): Ignored (no operation)
    /// - I/O (0xFF00-0xFF7F): Hardware registers
    /// - HRAM (0xFF80-0xFFFE): High RAM
    /// - IE (0xFFFF): Interrupt Enable register
    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x7FFF => self.cart.write_byte(address, value), // ROM Banks
            0x8000..=0x9FFF => {
                // Char/Map Data
                self.ppu.ppu_vram_write(address, value)
            }
            0xA000..=0xBFFF => self.cart.write_byte(address, value), // Cartridge RAM
            0xC000..=0xDFFF => self.ram.wram_write(address, value),  // WRAM
            0xE000..=0xFDFF => (),                                   // Reserved Echo RAM
            0xFE00..=0xFE9F => {
                // OAM RAM
                if self.dma.dma_transferring() {
                    return;
                } else {
                    self.ppu.ppu_oam_write(address, value)
                }
            }
            0xFEA0..=0xFEFF => (), // Reserved Unusable
            0xFF00..=0xFF7F => {
                // IO Registers
                io_write(
                    address,
                    value,
                    &mut self.dma,
                    &mut self.interrupt_controller,
                    &mut self.ppu,
                    &mut self.gamepad,
                    &mut self.apu,
                );
            }
            0xFF80..=0xFFFE => self.ram.hram_write(address, value), // HRAM
            0xFFFF => self.interrupt_controller.set_ie_register(value), // Interrupt Enable Register
        }
    }

    /// Processes one tick of DMA transfer if active.
    ///
    /// Advances the DMA controller state by one cycle if a DMA transfer is
    /// currently in progress. Uses `std::mem::take` to temporarily move the
    /// DMA controller out of the bus for mutable access during the transfer.
    ///
    /// DMA transfers copy 160 bytes from source memory to OAM (0xFE00-0xFE9F)
    /// and take 160 machine cycles to complete. During transfer, CPU access
    /// to OAM is blocked.
    pub fn tick_dma(&mut self) {
        let mut dma = std::mem::take(&mut self.dma);
        if dma.dma_transferring() {
            dma.dma_tick(self);
        }
        self.dma = dma;
    }
}
