/*
  menu/rom_catalog.rs
  Info: ROM file catalog and management system
  Description: Scans and organizes Game Boy ROM files from the filesystem, providing
              metadata extraction and categorization. Separates game ROMs from test ROMs
              and detects battery-backed cartridges for save game support.

  ROM Detection:
    - Scans specified directories for .gb and .gbc files
    - Extracts file metadata (name, size, path)
    - Detects battery-backed cartridges by reading cartridge type byte
    - Sorts ROMs alphabetically for consistent display

  Directory Structure:
    - game_roms/: Commercial games and homebrew
    - test_roms/: Test ROMs for emulator validation

  Battery Detection:
    - Reads cartridge type byte at ROM offset 0x147
    - Identifies MBC types with battery backup (0x03, 0x06, 0x09, 0x0F-0x13, 0xFC-0xFF)
    - Used to indicate save game support in UI
*/

use std::fs;
use std::path::{Path, PathBuf};

use crate::menu::ROMTab;

/// Represents a Game Boy ROM file with metadata.
///
/// Contains information about a ROM file including its location,
/// size, and whether it supports battery-backed saves.
#[derive(Debug, Clone, PartialEq)]
pub struct ROM {
    /// Display name of the ROM (filename with extension)
    pub name: String,
    /// Full filesystem path to the ROM file
    pub path: String,
    /// Size of the ROM file in bytes
    pub file_size: u64,
    /// Whether the cartridge has battery-backed RAM for saves
    pub is_battery_backed: bool,
}

impl ROM {
    /// Parses a ROM file from the filesystem and extracts metadata.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the ROM file to parse
    ///
    /// # Returns
    ///
    /// `Some(ROM)` if the file is a valid Game Boy ROM, `None` otherwise.
    ///
    /// # Notes
    ///
    /// - Only accepts .gb and .gbc file extensions
    /// - Reads the cartridge type byte to detect battery backup
    /// - Returns `None` if file cannot be read or is invalid
    fn parse_rom_file(path: &Path) -> Option<ROM> {
        let extension = path.extension()?.to_str()?;
        if extension != "gb" && extension != "gbc" {
            return None;
        }

        let file_name = path.file_name()?.to_str()?;
        let metadata = fs::metadata(path).ok()?;

        // Read first few bytes to check for battery-backed RAM
        let file_content = fs::read(path).ok()?;
        let is_battery_backed = if file_content.len() >= 0x149 {
            let cart_type = file_content[0x147];
            matches!(cart_type, 0x03 | 0x06 | 0x09 | 0x0F..=0x13 | 0xFC..=0xFF)
        } else {
            false
        };

        Some(ROM {
            name: file_name.to_string(),
            path: path.to_str()?.to_string(),
            file_size: metadata.len(),
            is_battery_backed,
        })
    }
}

/// Catalog of available Game Boy ROM files organized by type.
///
/// Manages collections of game ROMs and test ROMs, providing
/// organized access to available ROM files for the menu system.
pub struct ROMCatalog {
    /// Collection of game ROMs (commercial and homebrew)
    pub game_roms: Vec<ROM>,
    /// Collection of test ROMs for emulator validation
    pub test_roms: Vec<ROM>,
}

impl ROMCatalog {
    /// Creates a new ROM catalog by scanning the specified directory.
    ///
    /// # Arguments
    ///
    /// * `roms_dir` - Base directory containing game_roms/ and test_roms/ subdirectories
    ///
    /// # Returns
    ///
    /// A new `ROMCatalog` with all discovered ROMs sorted alphabetically.
    ///
    /// # Directory Structure
    ///
    /// Expected structure:
    /// ```text
    /// roms_dir/
    ///   ├── game_roms/
    ///   │   ├── Pokemon Red.gb
    ///   │   └── Tetris.gb
    ///   └── test_roms/
    ///       ├── cpu_instrs.gb
    ///       └── mem_timing.gb
    /// ```
    pub fn new(roms_dir: &PathBuf) -> Self {
        let mut catalog = ROMCatalog {
            game_roms: Vec::new(),
            test_roms: Vec::new(),
        };

        // Collect games from the game roms dir
        let game_roms_dir = PathBuf::from(roms_dir).join("game_roms");
        if let Ok(entries) = fs::read_dir(&game_roms_dir) {
            for entry in entries.flatten() {
                if let Some(rom) = ROM::parse_rom_file(&entry.path()) {
                    catalog.game_roms.push(rom);
                }
            }
        }

        // Collect test roms from the test roms dir
        let test_roms_dir = PathBuf::from(roms_dir).join("test_roms");
        if let Ok(entries) = fs::read_dir(&test_roms_dir) {
            for entry in entries.flatten() {
                if let Some(rom) = ROM::parse_rom_file(&entry.path()) {
                    catalog.test_roms.push(rom);
                }
            }
        }

        // Sort roms alphabetically
        catalog.game_roms.sort_by_key(|g| g.name.to_lowercase());
        catalog.test_roms.sort_by_key(|g| g.name.to_lowercase());

        catalog
    }

    /// Returns the total number of ROMs in the catalog.
    ///
    /// # Returns
    ///
    /// Sum of game ROMs and test ROMs.
    pub fn len(&self) -> usize {
        self.game_roms.len().wrapping_add(self.test_roms.len())
    }

    /// Retrieves a ROM by its tab type and index.
    ///
    /// # Arguments
    ///
    /// * `rom_type` - The ROM tab (GameRoms or TestRoms) and index
    ///
    /// # Returns
    ///
    /// Reference to the ROM if found, `None` if index is out of bounds.
    pub fn get_rom(&self, rom_type: &ROMTab) -> Option<&ROM> {
        return match rom_type {
            ROMTab::GameRoms(idx) => self.game_roms.get(*idx),
            ROMTab::TestRoms(idx) => self.test_roms.get(*idx),
        };
    }

    /// Finds the index of a ROM in the catalog.
    ///
    /// # Arguments
    ///
    /// * `rom` - The ROM to search for
    ///
    /// # Returns
    ///
    /// The index of the ROM in its respective collection, or `None` if not found.
    ///
    /// # Notes
    ///
    /// Searches game ROMs first, then test ROMs.
    pub fn get_roms_idx(&self, rom: &ROM) -> Option<usize> {
        // Look for the rom in the game roms
        if let Some(idx) = self.game_roms.iter().position(|r| r == rom) {
            return Some(idx);
        }

        // Look for the rom in the test roms
        if let Some(idx) = self.test_roms.iter().position(|r| r == rom) {
            return Some(idx);
        }

        None
    }
}
