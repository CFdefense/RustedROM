/*
  hdw/ram.rs
  Info: Random Access Memory controller for Game Boy internal RAM
  Description: The ram module implements Work RAM (WRAM) and High RAM (HRAM) management.
              Provides fast access to system memory with proper address mapping and
              echo RAM handling for accurate Game Boy memory behavior.

  RAM Struct Members:
    wram: Work RAM Array - 8KB internal RAM for game data, variables, and stack operations
    hram: High RAM Array - 127 bytes of zero-page RAM for critical, fast-access code

  Memory Regions:
    Work RAM (WRAM):
      - Physical Address: 0xC000-0xDFFF (8KB total)
      - Echo RAM Mapping: 0xE000-0xFDFF mirrors WRAM content
      - Bank 0: 0xC000-0xCFFF (4KB, always accessible)
      - Bank 1-7: 0xD000-0xDFFF (4KB, switchable on Game Boy Color)

    High RAM (HRAM):
      - Physical Address: 0xFF80-0xFFFE (127 bytes)
      - Zero-page access for interrupt handlers and critical code
      - Fastest memory access in the system
      - Not affected by DMA transfers

  Core Functions:
    RAM::new: Constructor - Initializes both RAM arrays with zero values
    wram_read: WRAM Reader - Reads from work RAM with echo mapping support
    wram_write: WRAM Writer - Writes to work RAM handling echo addresses
    hram_read: HRAM Reader - Fast access to high RAM with bounds checking
    hram_write: HRAM Writer - Fast write to high RAM with validation

  Echo RAM Implementation:
    - Echo addresses (0xE000-0xFDFF) automatically map to WRAM (0xC000-0xDDFF)
    - Transparent mapping maintains compatibility with Game Boy software
    - No additional memory allocated for echo region
    - Proper address translation maintains performance

  Performance Features:
    - Direct array access for maximum speed
    - Minimal overhead address translation
    - Bounds checking with panic on invalid access
    - Zero-page optimization for HRAM access patterns

  Hardware Accuracy:
    - Accurate memory sizes matching original Game Boy
    - Proper echo RAM behavior
    - HRAM isolation from DMA transfers
    - Work RAM bank switching preparation (for Game Boy Color)

  Error Handling:
    - Panic on invalid address access for debugging
    - Clear error messages with address information
    - Bounds validation for both read and write operations
    - Address mapping validation and error reporting

  Memory Layout Accuracy:
    - WRAM: Exactly 8KB as in original hardware
    - HRAM: Exactly 127 bytes (0xFF80-0xFFFE)
    - Echo RAM: Proper mapping without duplication
    - Address ranges match original Game Boy specifications
*/

use core::panic;

/// Game Boy RAM controller managing Work RAM and High RAM.
///
/// This structure implements the Game Boy's internal RAM system, consisting of:
/// - Work RAM (WRAM): 8KB of general-purpose memory at 0xC000-0xDFFF
/// - High RAM (HRAM): 127 bytes of fast zero-page memory at 0xFF80-0xFFFE
///
/// The controller handles echo RAM mapping (0xE000-0xFDFF mirrors WRAM) and
/// provides fast, direct memory access for game data, variables, and stack operations.
pub struct RAM {
    /// Work RAM array (8KB) - General-purpose memory for game data and stack.
    /// Maps to addresses 0xC000-0xDFFF with echo region at 0xE000-0xFDFF.
    wram: [u8; 0x2000],

    /// High RAM array (127 bytes) - Fast zero-page memory for critical code.
    /// Maps to addresses 0xFF80-0xFFFE, isolated from DMA transfers.
    hram: [u8; 0x80],
}

impl RAM {
    /// Creates a new RAM controller with zeroed memory.
    ///
    /// Initializes both Work RAM (8KB) and High RAM (127 bytes) with zero values,
    /// matching the power-on state of the Game Boy hardware.
    ///
    /// # Returns
    ///
    /// A new `RAM` instance with all memory initialized to 0x00.
    pub fn new() -> Self {
        RAM {
            wram: [0; 0x2000],
            hram: [0; 0x80],
        }
    }

    /// Reads a byte from Work RAM with echo RAM support.
    ///
    /// Handles both direct WRAM access (0xC000-0xDFFF) and echo RAM addresses
    /// (0xE000-0xFDFF). Echo addresses are automatically mapped to their
    /// corresponding WRAM locations by subtracting 0x2000.
    ///
    /// # Arguments
    ///
    /// * `address` - Memory address to read from (0xC000-0xDFFF or 0xE000-0xFDFF)
    ///
    /// # Returns
    ///
    /// The byte value at the specified address.
    ///
    /// # Panics
    ///
    /// Panics if the address is outside valid WRAM/echo ranges with detailed
    /// error information including original, mapped, and offset addresses.
    pub fn wram_read(&self, address: u16) -> u8 {
        // Handle echo RAM addresses (0xE000-0xFDFF) by mapping them to WRAM
        let mapped_address = if address >= 0xE000 && address <= 0xFDFF {
            // Echo RAM maps to WRAM: 0xE000 -> 0xC000, 0xFDFF -> 0xDDFF
            address - 0x2000
        } else {
            address
        };

        let offset_address = mapped_address - 0xC000;

        if offset_address >= 0x2000 {
            panic!(
                "INVALID WRAM ADDRESS: {:04X} (mapped: {:04X}, offset: {:04X})",
                address, mapped_address, offset_address
            )
        }

        self.wram[offset_address as usize]
    }

    /// Writes a byte to Work RAM with echo RAM support.
    ///
    /// Handles both direct WRAM access (0xC000-0xDFFF) and echo RAM addresses
    /// (0xE000-0xFDFF). Echo addresses are automatically mapped to their
    /// corresponding WRAM locations, ensuring writes to echo addresses affect
    /// the actual WRAM.
    ///
    /// # Arguments
    ///
    /// * `address` - Memory address to write to (0xC000-0xDFFF or 0xE000-0xFDFF)
    /// * `value` - Byte value to write
    ///
    /// # Panics
    ///
    /// Panics if the address is outside valid WRAM/echo ranges with detailed
    /// error information including original, mapped, and offset addresses.
    pub fn wram_write(&mut self, address: u16, value: u8) {
        // Handle echo RAM addresses (0xE000-0xFDFF) by mapping them to WRAM
        let mapped_address = if address >= 0xE000 && address <= 0xFDFF {
            // Echo RAM maps to WRAM: 0xE000 -> 0xC000, 0xFDFF -> 0xDDFF
            address - 0x2000
        } else {
            address
        };

        let offset_address = mapped_address - 0xC000;

        if offset_address >= 0x2000 {
            panic!(
                "INVALID WRAM ADDRESS: {:04X} (mapped: {:04X}, offset: {:04X})",
                address, mapped_address, offset_address
            )
        }

        self.wram[offset_address as usize] = value;
    }

    /// Reads a byte from High RAM (zero-page memory).
    ///
    /// Provides fast access to the 127-byte High RAM region used for critical
    /// code and interrupt handlers. HRAM is isolated from DMA transfers and
    /// offers the fastest memory access in the system.
    ///
    /// # Arguments
    ///
    /// * `address` - Memory address to read from (0xFF80-0xFFFE)
    ///
    /// # Returns
    ///
    /// The byte value at the specified HRAM address.
    ///
    /// # Panics
    ///
    /// Panics if the address is outside the valid HRAM range (0xFF80-0xFFFE).
    pub fn hram_read(&self, address: u16) -> u8 {
        if address < 0xFF80 || address > 0xFFFE {
            panic!("INVALID HRAM ADDRESS: {:04X}", address);
        }

        let offset_address = address - 0xFF80;
        self.hram[offset_address as usize]
    }

    /// Writes a byte to High RAM (zero-page memory).
    ///
    /// Provides fast write access to the 127-byte High RAM region. HRAM is
    /// commonly used for interrupt handlers and time-critical code due to
    /// its fast access speed and isolation from DMA operations.
    ///
    /// # Arguments
    ///
    /// * `address` - Memory address to write to (0xFF80-0xFFFE)
    /// * `value` - Byte value to write
    ///
    /// # Panics
    ///
    /// Panics if the address is outside the valid HRAM range (0xFF80-0xFFFE).
    pub fn hram_write(&mut self, address: u16, value: u8) {
        if address < 0xFF80 || address > 0xFFFE {
            panic!("INVALID HRAM ADDRESS: {:04X}", address);
        }

        let offset_address = address - 0xFF80;
        self.hram[offset_address as usize] = value;
    }
}
