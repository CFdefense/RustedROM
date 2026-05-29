use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ROM {
    pub name: String,
    pub path: String,
    pub file_size: u64,
    pub is_battery_backed: bool,
    pub is_test_rom: bool,
}

pub struct ROMCatalog {
    pub game_roms: Vec<ROM>,
    pub test_roms: Vec<ROM>,
    pub rom_dir: Path,
}

impl ROMCatalog {
    pub fn new(roms_dir: &str) -> Self {
        let mut games = Vec::new();
        
        // Collect games from the game roms dir 
        let game_roms_dir = Path::new(roms_dir).join("game_roms");
        if let Ok(entries) = fs::read_dir(&game_roms_dir) {
            for entry in entries.flatten() {
                if let Some(game_info) = Self::process_game_file(&entry.path(), false) {
                        games.push(game_info);
                    }
            }
        }
        
        // Collect test roms from the test roms dir 
        let test_roms_dir = Path::new(roms_dir).join("test_roms");
        if let Ok(entries) = fs::read_dir(&test_roms_dir) {
                for entry in entries.flatten() {
                if let Some(game_info) = Self::process_game_file(&entry.path(), true) {
                            games.push(game_info);
                }
            }
        }
        
        // Sort games alphabetically
        games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        games
    }
    
    fn process_game_file(path: &Path, is_test_rom: bool) -> Option<Game> {
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
        
        Some(Game {
            name: file_name.to_string(),
            path: path.to_str()?.to_string(),
            file_size: metadata.len(),
            is_battery_backed,
            is_test_rom,
        })
    }
} 
