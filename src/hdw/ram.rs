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

pub struct RAM {
    wram: [u8; 0x2000],
    hram: [u8; 0x80],
}

impl RAM {
    // Constructor
    pub fn new() -> Self {
        RAM {
            wram: [0; 0x2000],
            hram: [0; 0x80],
        }
    }

    // Method to read from wram
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

    // Method to write to wram
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

    // Method to read from hram
    pub fn hram_read(&self, address: u16) -> u8 {
        if address < 0xFF80 || address > 0xFFFE {
            panic!("INVALID HRAM ADDRESS: {:04X}", address);
        }

        let offset_address = address - 0xFF80;
        self.hram[offset_address as usize]
    }

    // Method to write to hram
    pub fn hram_write(&mut self, address: u16, value: u8) {
        if address < 0xFF80 || address > 0xFFFE {
            panic!("INVALID HRAM ADDRESS: {:04X}", address);
        }

        let offset_address = address - 0xFF80;
        self.hram[offset_address as usize] = value;
    }
}
