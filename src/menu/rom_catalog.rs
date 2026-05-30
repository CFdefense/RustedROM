use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ROM {
    pub name: String,
    pub path: String,
    pub file_size: u64,
    pub is_battery_backed: bool,
}

impl ROM {
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

pub struct ROMCatalog {
    pub game_roms: Vec<ROM>,
    pub test_roms: Vec<ROM>,
    pub rom_dir: PathBuf,
}

impl ROMCatalog {
    pub fn new(roms_dir: &PathBuf) -> Self {
        let mut catalog = ROMCatalog {
            game_roms: Vec::new(),
            test_roms: Vec::new(),
            rom_dir: roms_dir.clone(),
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

    pub fn len(&self) -> usize {
        self.game_roms.len().wrapping_add(self.test_roms.len())
    }
}
