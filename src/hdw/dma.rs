/*
  hdw/dma.rs
  Info: Direct Memory Access controller for Game Boy sprite data transfers
  Description: The dma module implements the Game Boy's DMA controller for high-speed transfer
              of sprite attribute data from main memory to OAM (Object Attribute Memory).
              Provides cycle-accurate transfer timing and proper access restrictions.

  DMA Struct Members:
    active: Transfer Status - Indicates if DMA transfer is currently in progress
    current_byte: Transfer Progress - Current byte offset being transferred (0-159)
    byte_value: Source Page - High byte of source address (source = byte_value * 0x100)
    start_delay: Startup Delay - Initial delay cycles before transfer begins (2 cycles)

  DMA Transfer Process:
    1. Game writes source page to DMA register (FF46)
    2. DMA controller starts with 2-cycle startup delay
    3. Transfers 160 bytes (0xA0) from source to OAM
    4. Each byte transfer takes 1 cycle
    5. Total transfer time: 162 cycles (2 startup + 160 transfer)

  Core Functions:
    DMA::new: Constructor - Creates DMA controller with default inactive state
    dma_start: Transfer Initiator - Begins DMA transfer with specified source page
    dma_tick: Transfer Engine - Processes one cycle of DMA transfer operation
    dma_transferring: Status Query - Returns true if DMA transfer is currently active

  Memory Layout:
    Source Address: (byte_value * 0x100) + current_byte
    - Supported ranges: 0x0000-0xDFFF (ROM, VRAM, WRAM)
    - Forbidden ranges: 0xE000-0xFFFF (Echo RAM, OAM, I/O, HRAM)

    Destination: OAM (0xFE00-0xFE9F)
    - 160 bytes total (40 sprites × 4 bytes each)
    - Direct write to PPU's Object Attribute Memory

  Access Restrictions:
    During DMA Transfer:
    - CPU cannot access OAM (reads return 0xFF)
    - CPU can still access other memory regions
    - DMA has priority over CPU for OAM access
    - Game execution continues during transfer

  Timing Accuracy:
    - 2-cycle startup delay before first byte transfer
    - 1 cycle per byte transferred (160 cycles for full transfer)
    - Total transfer time: 162 cycles
    - Transfer occurs at CPU speed (1MHz)

  Hardware Integration:
    - Triggered by write to DMA register (FF46)
    - Integrates with bus for source memory reading
    - Coordinates with PPU for OAM writing
    - Provides status for CPU access control

  Performance Features:
    - High-speed batch transfer (faster than CPU copy loops)
    - Non-blocking transfer (CPU continues execution)
    - Efficient sprite data loading for games
    - Minimal overhead transfer management

  Sprite Data Format:
    Each sprite consists of 4 bytes in OAM:
    - Byte 0: Y position (screen coordinate + 16)
    - Byte 1: X position (screen coordinate + 8)
    - Byte 2: Tile number (sprite pattern index)
    - Byte 3: Attributes (palette, flip, priority flags)
*/

use crate::hdw::bus::BUS;

/// Direct Memory Access controller for high-speed sprite data transfers.
///
/// The DMA controller manages transfers of 160 bytes from main memory to
/// Object Attribute Memory (OAM) for sprite rendering. Transfers take 162
/// cycles total (2 startup + 160 transfer) and block CPU access to OAM
/// during the transfer period.
///
/// # Transfer Process
///
/// 1. Write source page to DMA register (0xFF46)
/// 2. Wait 2 cycles for startup delay
/// 3. Transfer 160 bytes (one per cycle) to OAM
/// 4. CPU access to OAM blocked during transfer
///
/// # Memory Layout
///
/// - Source: (byte_value × 0x100) + current_byte
/// - Destination: 0xFE00 + current_byte
/// - Valid sources: 0x0000-0xDFFF (ROM, VRAM, WRAM)
#[derive(Default)]
pub struct DMA {
    /// Transfer active flag - true when DMA transfer is in progress.
    pub active: bool,

    /// Current byte offset being transferred (0-159, or 0xA0 when complete).
    pub current_byte: u8,

    /// Source page high byte - source address is (byte_value × 0x100) + offset.
    pub byte_value: u8,

    /// Startup delay counter - decrements from 2 to 0 before transfer begins.
    pub start_delay: u8,
}

impl DMA {
    /// Creates a new DMA controller in inactive state.
    ///
    /// Initializes the DMA controller with all fields set to their default
    /// values (inactive, no transfer in progress).
    ///
    /// # Returns
    ///
    /// A new `DMA` instance with `active` set to false.
    pub fn new() -> Self {
        Default::default()
    }

    /// Initiates a DMA transfer from the specified source page.
    ///
    /// Starts a DMA transfer that will copy 160 bytes from the source address
    /// (start × 0x100) to OAM at 0xFE00-0xFE9F. The transfer begins after a
    /// 2-cycle startup delay.
    ///
    /// # Arguments
    ///
    /// * `start` - Source page high byte (source address = start × 0x100)
    ///
    /// # Valid Source Ranges
    ///
    /// - 0x00-0x7F: ROM (0x0000-0x7FFF)
    /// - 0x80-0x9F: VRAM (0x8000-0x9FFF)
    /// - 0xA0-0xBF: Cartridge RAM (0xA000-0xBFFF)
    /// - 0xC0-0xDF: Work RAM (0xC000-0xDFFF)
    ///
    /// # Effects
    ///
    /// - Sets transfer to active state
    /// - Resets byte counter to 0
    /// - Sets 2-cycle startup delay
    /// - Blocks CPU access to OAM until complete
    pub fn dma_start(&mut self, start: u8) {
        self.active = true;
        self.current_byte = 0;
        self.byte_value = start;
        self.start_delay = 2;
    }

    /// Processes one cycle of DMA transfer.
    ///
    /// Advances the DMA transfer state by one cycle. During the startup delay,
    /// no transfer occurs. After the delay, one byte is transferred per cycle
    /// from source memory to OAM until all 160 bytes are complete.
    ///
    /// # Arguments
    ///
    /// * `bus` - Mutable reference to the memory bus for reading source and writing OAM
    ///
    /// # Returns
    ///
    /// `true` if the transfer is still active after this tick, `false` if
    /// inactive or just completed.
    ///
    /// # Transfer Timing
    ///
    /// - Cycles 0-1: Startup delay (no transfer)
    /// - Cycles 2-161: Transfer one byte per cycle
    /// - Cycle 162: Transfer complete, returns false
    pub fn dma_tick(&mut self, bus: &mut BUS) -> bool {
        if !self.active {
            return false;
        }

        if self.start_delay > 0 {
            self.start_delay -= 1;
            return false;
        }

        let source_address = ((self.byte_value as u16) * 0x100) + self.current_byte as u16;
        let value = bus.read_byte(None, source_address);
        bus.ppu.ppu_oam_write(self.current_byte as u16, value);

        self.current_byte = self.current_byte + 1;
        self.active = self.current_byte < 0xA0;

        self.active
    }

    /// Checks if a DMA transfer is currently in progress.
    ///
    /// Returns the active status of the DMA controller. When true, CPU access
    /// to OAM (0xFE00-0xFE9F) is blocked and reads return 0xFF.
    ///
    /// # Returns
    ///
    /// `true` if DMA transfer is active (including startup delay), `false` otherwise.
    pub fn dma_transferring(&self) -> bool {
        self.active
    }
}
