use std::path::PathBuf;

use crate::menu::rom_catalog;

use super::color_palette::ColorPalette;
use super::rom_catalog::{ROMCatalog, ROM};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuState {
    MainMenu,
    Credits,
    ROMSelection,
    PaletteSelection,
    ROMOpen(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ROMTab {
    GameRoms,
    TestRoms,
}

pub struct Menu {
    pub rom_catalog: ROMCatalog,
    pub current_state: MenuState,
    pub selected_main_option: usize, // 0 = Start, 1 = Palette, 2 = Credits
    pub selected_rom_index: usize,
    pub selected_palette_index: usize,
    pub current_palette: ColorPalette,
    pub available_palettes: Vec<ColorPalette>,
    pub scroll_offset: usize,
    pub max_visible_games: usize,
    pub credits_scroll: f32,
    pub animation_time: f32,
    pub debug: bool,
    pub current_tab: ROMTab,
}

impl Menu {
    pub fn new(rom_dir: PathBuf, debug: bool) -> Self {
        Menu {
            rom_catalog: ROMCatalog::new(&rom_dir),
            current_state: MenuState::MainMenu,
            selected_main_option: 0,
            selected_rom_index: 0,
            selected_palette_index: 0,
            current_palette: ColorPalette::ClassicGameBoy,
            available_palettes: ColorPalette::all_palettes(),
            scroll_offset: 0,
            max_visible_games: 12,
            credits_scroll: 0.0,
            animation_time: 0.0,
            debug,
            current_tab: ROMTab::GameRoms,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.animation_time += delta_time;
    }

    pub fn navigate_up(&mut self) {
        match self.current_state {
            MenuState::MainMenu => {
                if self.selected_main_option > 0 {
                    self.selected_main_option -= 1;
                }
            }
            MenuState::ROMSelection => {
                if self.selected_rom_index > 0 {
                    self.selected_rom_index -= 1;
                    if self.selected_rom_index < self.scroll_offset {
                        self.scroll_offset = self.selected_rom_index;
                    }
                }
            }
            MenuState::PaletteSelection => {
                if self.selected_palette_index > 0 {
                    self.selected_palette_index -= 1;
                }
            }
            _ => {}
        }
    }

    pub fn navigate_down(&mut self) {
        match self.current_state {
            MenuState::MainMenu => {
                if self.selected_main_option < 2 {
                    self.selected_main_option += 1;
                }
            }
            MenuState::ROMSelection => {
                let max_index = self.get_tab_roms().len().saturating_sub(1);
                if self.selected_rom_index < max_index {
                    self.selected_rom_index += 1;
                    if self.selected_rom_index >= self.scroll_offset + self.max_visible_games {
                        self.scroll_offset = self.selected_rom_index + 1 - self.max_visible_games;
                    }
                }
            }
            MenuState::PaletteSelection => {
                if self.selected_palette_index < self.available_palettes.len().saturating_sub(1) {
                    self.selected_palette_index += 1;
                }
            }
            _ => {}
        }
    }

    pub fn select(&mut self) -> Option<String> {
        match self.current_state {
            MenuState::MainMenu => {
                match self.selected_main_option {
                    0 => {
                        // Start
                        self.current_state = MenuState::ROMSelection;
                        None
                    }
                    1 => {
                        // Palette
                        self.current_state = MenuState::PaletteSelection;
                        None
                    }
                    2 => {
                        // Credits
                        self.current_state = MenuState::Credits;
                        self.credits_scroll = 0.0;
                        None
                    }
                    _ => None,
                }
            }
            MenuState::ROMSelection => {
                // Get filtered roms for current tab
                let filtered_roms = self.get_tab_roms();

                // Get the rom at the filtered index
                if let Some(rom) = filtered_roms.get(self.selected_rom_index) {
                    let rom_path = rom.path.clone();
                    self.current_state = MenuState::ROMOpen(rom_path.clone());
                    Some(rom_path)
                } else {
                    None
                }
            }
            MenuState::PaletteSelection => {
                if let Some(palette) = self.available_palettes.get(self.selected_palette_index) {
                    self.current_palette = palette.clone();
                    println!("Selected palette: {}", palette.get_name());
                }
                None
            }
            _ => None,
        }
    }

    pub fn back(&mut self) {
        match self.current_state {
            MenuState::Credits => {
                self.current_state = MenuState::MainMenu;
            }
            MenuState::ROMSelection => {
                self.current_state = MenuState::MainMenu;
            }
            MenuState::PaletteSelection => {
                self.current_state = MenuState::MainMenu;
            }
            MenuState::ROMOpen(_) => {
                self.current_state = MenuState::ROMSelection;
            }
            _ => {}
        }
    }

    pub fn exit_rom(&mut self) {
        if matches!(self.current_state, MenuState::ROMOpen(_)) {
            self.current_state = MenuState::ROMSelection;
        }
    }

    pub fn get_tab_roms(&self) -> Vec<&ROM> {
        match self.current_tab {
            ROMTab::GameRoms => self.rom_catalog.game_roms.iter().collect(),
            ROMTab::TestRoms => self.rom_catalog.test_roms.iter().collect(),
        }
    }
    pub fn get_visible_roms(&self) -> Vec<(usize, &ROM)> {
        self.get_tab_roms()
            .into_iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.max_visible_games)
            .collect()
    }

    pub fn switch_tab(&mut self) {
        // Store current selection before switching
        let current_selection = self.get_selected_rom();
        let current_name = current_selection.map(|rom| rom.name.clone());

        // Switch tab
        self.current_tab = match self.current_tab {
            ROMTab::GameRoms => ROMTab::TestRoms,
            ROMTab::TestRoms => ROMTab::GameRoms,
        };

        // Reset selection and scroll
        self.selected_rom_index = 0;
        self.scroll_offset = 0;

        // If we had a selection, try to find a rom with the same name in the new tab
        if let Some(prev_name) = current_name {
            if let Some(idx) = self
                .get_tab_roms()
                .iter()
                .position(|rom| rom.name == prev_name)
            {
                self.selected_rom_index = idx;
            }
        }
    }

    pub fn get_selected_rom(&self) -> Option<&ROM> {
        // Get all roms for the current tab
        let filtered_roms = self.get_tab_roms();

        // Get the rom at the selected index
        filtered_roms.get(self.selected_rom_index).copied()
    }
}
