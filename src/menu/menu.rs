use std::path::PathBuf;

use sdl2::pixels::Color;
use sdl2::surface::Surface;
use sdl2::rect::Rect;

use super::color_palette::ColorPalette;
use super::rom_catalog::{ROMCatalog, ROM};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuState {
    MainMenu(usize),
    Credits(f32),
    ROMSelection(ROMTab),
    PaletteSelection(usize),
    ROMOpen(ROM),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ROMTab {
    GameRoms(usize),
    TestRoms(usize),
}

pub struct Menu {
    pub rom_catalog: ROMCatalog,
    pub current_state: MenuState,
    pub current_palette: ColorPalette,
    pub available_palettes: Vec<ColorPalette>,
    pub scroll_offset: usize,
    pub max_visible_roms: usize,
    pub animation_time: f32,
    pub debug: bool,
    pub screen_width: u32,
    pub screen_height: u32,
}

impl Menu {
    // Colors for the menu theme
    const BG_COLOR: Color = Color::RGB(20, 20, 30); // Dark blue background
    const PRIMARY_COLOR: Color = Color::RGB(100, 200, 255); // Light blue for titles
    const SECONDARY_COLOR: Color = Color::RGB(80, 160, 200); // Medium blue for text
    const SELECTED_COLOR: Color = Color::RGB(255, 200, 100); // Orange for selected items
    const BATTERY_COLOR: Color = Color::RGB(100, 255, 100); // Green for battery backed games
    const CREDITS_COLOR: Color = Color::RGB(180, 180, 180); // Light gray for credits

    pub fn new(rom_dir: PathBuf, screen_width: u32, screen_height: u32, debug: bool) -> Self {
        Menu {
            rom_catalog: ROMCatalog::new(&rom_dir),
            current_state: MenuState::MainMenu(0),
            current_palette: ColorPalette::ClassicGameBoy,
            available_palettes: ColorPalette::all_palettes(),
            scroll_offset: 0,
            max_visible_roms: 12,
            animation_time: 0.0,
            screen_width,
            screen_height,
            debug,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.animation_time += delta_time;
    }

    pub fn navigate_up(&mut self) {
        match &mut self.current_state {
            MenuState::MainMenu(idx) => {
                if *idx > 0 {
                    *idx -= 1;
                }
            }
            MenuState::ROMSelection(tab) => match tab {
                ROMTab::GameRoms(idx) => {
                    if *idx > 0 {
                        *idx -= 1;
                        if *idx < self.scroll_offset {
                            self.scroll_offset = *idx;
                        }
                    }
                }
                ROMTab::TestRoms(idx) => {
                    if *idx > 0 {
                        *idx -= 1;
                        if *idx < self.scroll_offset {
                            self.scroll_offset = *idx;
                        }
                    }
                }
            },
            MenuState::PaletteSelection(idx) => {
                if *idx > 0 {
                    *idx -= 1;
                }
            }
            _ => {}
        }
    }

    pub fn navigate_down(&mut self) {
        match &mut self.current_state {
            MenuState::MainMenu(idx) => {
                if *idx < 2 {
                    *idx += 1;
                }
            }
            MenuState::ROMSelection(tab) => {
                let max_index = self.rom_catalog.len().saturating_sub(1);
                match tab {
                    ROMTab::GameRoms(idx) => {
                        if *idx < max_index {
                            *idx += 1;
                            if *idx >= self.scroll_offset + self.max_visible_roms {
                                self.scroll_offset = *idx + 1 - self.max_visible_roms;
                            }
                        }
                    }
                    ROMTab::TestRoms(idx) => {
                        if *idx < max_index {
                            *idx += 1;
                            if *idx >= self.scroll_offset + self.max_visible_roms {
                                self.scroll_offset = *idx + 1 - self.max_visible_roms;
                            }
                        }
                    }
                }
            }
            MenuState::PaletteSelection(idx) => {
                if *idx < self.available_palettes.len().saturating_sub(1) {
                    *idx += 1;
                }
            }
            _ => {}
        }
    }

    pub fn navigate_horizontal(&mut self) {
        match &mut self.current_state {
            MenuState::ROMSelection(tab) => match tab {
                ROMTab::GameRoms(_) => {
                    self.current_state = MenuState::ROMSelection(ROMTab::TestRoms(0))
                }
                ROMTab::TestRoms(_) => {
                    self.current_state = MenuState::ROMSelection(ROMTab::GameRoms(0))
                }
            },
            _ => return, // no current horizontal movement functionality for other state
        }
    }

    pub fn select(&mut self) {
        match &self.current_state {
            MenuState::MainMenu(idx) => {
                match *idx {
                    0 => {
                        // ROM page selected
                        self.current_state = MenuState::ROMSelection(ROMTab::GameRoms(0));
                    }
                    1 => {
                        // Palette page selected
                        self.current_state = MenuState::PaletteSelection(0);
                    }
                    2 => {
                        // Credits page selected
                        self.current_state = MenuState::Credits(0.0);
                    }
                    // Menu out of index -> panic
                    _ => panic!("Menu out of index"),
                }
            }
            MenuState::ROMSelection(tab) => {
                let idx = match tab {
                    ROMTab::GameRoms(idx) => idx,
                    ROMTab::TestRoms(idx) => idx,
                };

                // Get filtered roms for current tab
                let filtered_roms = self.get_tab_roms();

                // Get the rom at the filtered index
                if let Some(&rom) = filtered_roms.get(*idx) {
                    self.current_state = MenuState::ROMOpen(rom.clone()) // cheap clone so clone
                }
            }
            MenuState::PaletteSelection(idx) => {
                // look up the correct palette and set it accordingly
                if let Some(palette) = self.available_palettes.get(*idx) {
                    self.current_palette = palette.clone();
                }
            }

            _ => {}
        }
    }

    pub fn back(&mut self) {
        match self.current_state {
            MenuState::Credits(_) => {
                self.current_state = MenuState::MainMenu(0);
            }
            MenuState::ROMSelection(_) => {
                self.current_state = MenuState::MainMenu(0);
            }
            MenuState::PaletteSelection(0) => {
                self.current_state = MenuState::MainMenu(0);
            }
            MenuState::ROMOpen(_) => {
                self.current_state = MenuState::ROMSelection(ROMTab::GameRoms(0));
            }
            _ => {}
        }
    }

    pub fn get_tab_roms(&self) -> Vec<&ROM> {
        match &self.current_state {
            MenuState::ROMSelection(tab) => match tab {
                ROMTab::GameRoms(_) => self.rom_catalog.game_roms.iter().collect(),
                ROMTab::TestRoms(_) => self.rom_catalog.test_roms.iter().collect(),
            },
            _ => vec![],
        }
    }

    pub fn get_visible_roms(&self) -> Vec<(usize, &ROM)> {
        self.get_tab_roms()
            .into_iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.max_visible_roms)
            .collect()
    }

    pub fn render(&mut self, surface: &mut Surface) {
        // Clear background
        surface.fill_rect(None, Self::BG_COLOR).unwrap();

        // Render the correct window
        match self.current_state {
            MenuState::MainMenu(_) => self.render_main_menu(surface),
            MenuState::Credits(_) => self.render_credits(surface),
            MenuState::ROMSelection(_) => self.render_game_selection(surface),
            MenuState::PaletteSelection(_) => self.render_palette_selection(surface),
            MenuState::ROMOpen(_) =>  return, // ROM Open -> Menu shouldnt be rendered
        }
    }

    fn render_main_menu(&self, surface: &mut Surface) {
        let center_x = self.screen_width as i32 / 2;
        let center_y = self.screen_height as i32 / 2;

        // Draw "RustedROM" title with ASCII art style - centered
        self.draw_title_text(surface, center_x, center_y - 130);

        // Draw subtitle - centered with more gap from ROM
        self.draw_text_centered(
            surface,
            "A Gameboy Emulator Written in Rust",
            center_x,
            center_y - 10,
            Self::SECONDARY_COLOR,
            2,
        );

        // Get the currently hovered menu option 
        let selected_index = match self.current_state {
            MenuState::MainMenu(idx) => idx,
            _ => return,
        };

        // If something is selected lets highlight it
        let start_color = if selected_index == 0 {
            Self::SELECTED_COLOR
        } else {
            Self::PRIMARY_COLOR
        };
        let palette_color = if selected_index == 1 {
            Self::SELECTED_COLOR
        } else {
            Self::PRIMARY_COLOR
        };
        let credits_color = if selected_index == 2 {
            Self::SELECTED_COLOR
        } else {
            Self::PRIMARY_COLOR
        };

        // Place each text element in a nice centered column
        let start_y = center_y + 40;
        let palette_y = center_y + 80;
        let credits_y = center_y + 120;

        // Always draw text in the same position (centered)
        self.draw_text_centered(surface, "START", center_x, start_y, start_color, 3);
        self.draw_text_centered(surface, "PALETTE", center_x, palette_y, palette_color, 3);
        self.draw_text_centered(surface, "CREDITS", center_x, credits_y, credits_color, 3);

        // Draw selection arrow separately to the left of selected option
        let arrow_offset = 100; // Increased distance from center to place arrow (more space)
        if selected_index == 0 {
            self.draw_text_centered(
                surface,
                ">",
                center_x - arrow_offset,
                start_y,
                Self::SELECTED_COLOR,
                3,
            );
        } else if selected_index == 1 {
            self.draw_text_centered(
                surface,
                ">",
                center_x - arrow_offset,
                palette_y,
                Self::SELECTED_COLOR,
                3,
            );
        } else if selected_index == 2 {
           self.draw_text_centered(
                surface,
                ">",
                center_x - arrow_offset,
                credits_y,
                Self::SELECTED_COLOR,
                3,
            );
        }

        // Show current palette selection
        let current_palette_text =
                format!("Current: {}", self.current_palette.get_name());
            self.draw_text_centered(
                surface,
                &current_palette_text,
                center_x,
                credits_y + 60,
                Self::SECONDARY_COLOR,
                1,
            );

            // Draw controls hint at bottom - centered
            self.draw_text_centered(
                surface,
            "Arrow Keys: Navigate  |  Enter: Select",
            center_x,
            self.screen_height as i32 - 30,
            Self::SECONDARY_COLOR,
            1,
        );
    }

    fn render_credits(&self, surface: &mut Surface) {
        let center_x = self.screen_width as i32 / 2;

        // Static credits content - start higher and use consistent spacing
        let mut y_offset = 40; // Start from near top
        let small_gap = 15; // Small gap between lines
        let medium_gap = 25; // Medium gap between sections
        let large_gap = 35; // Large gap for major sections

        // Title
        self.draw_text_centered(
            surface,
            "RustedROM",
            center_x,
            y_offset,
            Self::PRIMARY_COLOR,
            3,
        );
        y_offset += large_gap + 15; // Extra spacing after main title

        self.draw_text_centered(
            surface,
            "Game Boy Emulator",
            center_x,
            y_offset,
            Self::SECONDARY_COLOR,
            2,
        );
        y_offset += medium_gap;

        // Creator credit
        self.draw_text_centered(
            surface,
            "Created by Christian Farrell",
            center_x,
            y_offset,
            Self::CREDITS_COLOR,
            1,
        );
        y_offset += small_gap;

        self.draw_text_centered(
            surface,
            "Built with Rust & SDL2",
            center_x,
            y_offset,
            Self::CREDITS_COLOR,
            1,
        );
        y_offset += large_gap;

        // Features section
        self.draw_text_centered(
            surface,
            "=== FEATURES ===",
            center_x,
            y_offset,
            Self::SECONDARY_COLOR,
            2,
        );
        y_offset += medium_gap;

        let features = vec![
            "Complete Game Boy CPU emulation",
            "PPu with accurate timing",
            "Audio APU with 4 channels",
            "MBC1, MBC2 & MBC3 cartridge support",
            "Battery save system",
            "Real-time clock RTC support",
        ];

        for feature in features {
            self.draw_text_centered(surface, feature, center_x, y_offset, Self::CREDITS_COLOR, 1);
            y_offset += small_gap;
        }

        y_offset += medium_gap;

        // Thanks section
        self.draw_text_centered(
            surface,
            "=== THANKS ===",
            center_x,
            y_offset,
            Self::SECONDARY_COLOR,
            2,
        );
        y_offset += medium_gap;

        let thanks = vec![
            "Pan Docs for GB hardware docs",
            "Game Boy development community",
            "Rust & SDL2 contributors",
            "Professor Brian Gormanly",
        ];

        for thank in thanks {
            self.draw_text_centered(surface, thank, center_x, y_offset, Self::CREDITS_COLOR, 1);
            y_offset += small_gap;
        }

        y_offset += large_gap;

        // Final message
        self.draw_text_centered(
            surface,
            "Thank you for using RustedROM!",
            center_x,
            y_offset,
            Self::PRIMARY_COLOR,
            2,
        );

        // Draw back instruction - always at bottom
        self.draw_text_centered(
            surface,
            "Press Backspace to return",
            center_x,
            self.screen_height as i32 - 30,
            Self::SELECTED_COLOR,
            2,
        );
    }

    fn render_game_selection(&self, surface: &mut Surface) {
        // Split screen: left side for game list, right side for game info
        let split_x = self.screen_width * 3 / 5; // 60% for game list, 40% for info

        // Draw title with better positioning
        self.draw_text_centered(
            surface,
            "Select Game",
            self.screen_width as i32 / 2,
            25,
            Self::PRIMARY_COLOR,
            3,
        );

        // Draw tabs
        let tab_y = 60;
        let games_tab_x = 20;
        let test_roms_tab_x = games_tab_x + 150;

        // Draw tab backgrounds
        let tab_width = 140;
        let tab_height = 25;

        // Set the tab color
        let selected_tab = match self.current_state {
            MenuState::ROMSelection(tab) => tab,
            _ => return,
        };

        // Games tab
        let games_tab_color = if matches!(selected_tab, ROMTab::GameRoms(_)) {
            Self::SELECTED_COLOR
        } else {
            Self::SECONDARY_COLOR
        };
        let games_tab_rect = Rect::new(games_tab_x, tab_y, tab_width, tab_height);
        surface
            .fill_rect(
                games_tab_rect,
                Color::RGBA(games_tab_color.r, games_tab_color.g, games_tab_color.b, 30),
            )
            .unwrap();
        self.draw_text_centered(
            surface,
            "GAMES",
            games_tab_x + (tab_width as i32 / 2),
            tab_y + 5,
            games_tab_color,
            2,
        );

        // Test ROMs tab
        let test_roms_tab_color = if matches!(selected_tab, ROMTab::TestRoms(_)) {
            Self::SELECTED_COLOR
        } else {
            Self::SECONDARY_COLOR
        };
        let test_roms_tab_rect = Rect::new(test_roms_tab_x, tab_y, tab_width, tab_height);
        surface
            .fill_rect(
                test_roms_tab_rect,
                Color::RGBA(
                    test_roms_tab_color.r,
                    test_roms_tab_color.g,
                    test_roms_tab_color.b,
                    30,
                ),
            )
            .unwrap();
        self.draw_text_centered(
            surface,
            "TEST ROMS",
            test_roms_tab_x + (tab_width as i32 / 2),
            tab_y + 5,
            test_roms_tab_color,
            2,
        );

        // Draw game list on the left
        self.render_game_list(surface, split_x);

        // Draw game info on the right
        self.render_game_info(surface, split_x);

        // Draw controls with tab switching instruction
        let controls = "UP/DOWN: Navigate | LEFT/RIGHT: Switch List | ENTER: Launch | BACKSPACE: Back | ESC: Exit";
        self.draw_text_centered(
            surface,
            controls,
            self.screen_width as i32 / 2,
            self.screen_height as i32 - 15,
            Self::SECONDARY_COLOR,
            1,
        );
    }

    fn render_game_list(&self, surface: &mut Surface, split_x: u32) {
        let list_x = 20;
        let start_y = 100; // Increased to make room for tabs
        let line_height = 25;

        // Get the currently selected tab
        let current_tab = match self.current_state {
            MenuState::ROMSelection(tab) => tab,
            _ => return,
        };

        // Get the currently selected index of the tab
        let selected_rom_index= match current_tab {
            ROMTab::GameRoms(game_index) => game_index,
            ROMTab::TestRoms(test_index) => test_index,
        };

        let visible_roms = self.get_visible_roms();
        let total_roms = match current_tab {
            ROMTab::GameRoms(_) => self.rom_catalog.game_roms.len(),
            ROMTab::TestRoms(_) => self.rom_catalog.test_roms.len(),
        };

        if visible_roms.is_empty() {
            let empty_message = match current_tab {
                ROMTab::GameRoms(_) => {
                    "No games found!\nPlace .gb/.gbc files in 'roms/game_roms/' directory"
                }
                ROMTab::TestRoms(_)=> {
                    "No test ROMs found!\nPlace test ROMs in 'roms/test_roms/' directory"
                }
            };
            self.draw_text(
                surface,
                empty_message,
                list_x,
                start_y + 50,
                Self::CREDITS_COLOR,
                2,
            );
            return;
        }

        // Draw the selection on the correct rom
        for (i, (filtered_index, rom)) in visible_roms.iter().enumerate() {
            let y = start_y + (i as i32 * line_height);
            let is_selected = *filtered_index == selected_rom_index;

            // Draw selection highlight
            if is_selected {
                let highlight_rect =
                    Rect::new(list_x - 5, y - 3, split_x - 30, line_height as u32 - 2);
                surface
                    .fill_rect(highlight_rect, Color::RGBA(100, 200, 255, 30))
                    .unwrap();
            }

            // Draw selection arrow
            let arrow = if is_selected { ">" } else { " " };
            let arrow_color = if is_selected {
                Self::SELECTED_COLOR
            } else {
                Self::SECONDARY_COLOR
            };
            self.draw_text(surface, arrow, list_x, y, arrow_color, 2);

            // Draw rom name
            let name_color = if is_selected {
                Self::SELECTED_COLOR
            } else {
                Self::PRIMARY_COLOR
            };
            self.draw_text(surface, &rom.name, list_x + 20, y, name_color, 2);
        }

        // Draw scroll indicators if needed
        if self.scroll_offset > 0 {
            self.draw_text_centered(
                surface,
                "^ More ROMs above",
                split_x as i32 / 2,
                start_y - 5,
                Self::SECONDARY_COLOR,
                1,
            );
        }
        if self.scroll_offset + self.max_visible_roms < total_roms {
            let bottom_y = start_y + (self.max_visible_roms as i32 * line_height) + 5;
            self.draw_text_centered(
                surface,
                "v More ROMs below",
                split_x as i32 / 2,
                bottom_y,
                Self::SECONDARY_COLOR,
                1,
            );
        }
    }

    fn render_rom_info(
        &self,
        surface: &mut Surface,
        split_x: u32,
    ) {
        let info_x = split_x as i32 + 20;
        let start_y = 80;

        // Draw "ROM Info" header
        self.draw_text(
            surface,
            "ROM Info:",
            info_x,
            start_y - 30,
            Self::SECONDARY_COLOR,
            2,
        );

        if let Some(rom) = self.get_selected_game() {
            let mut y = start_y;
            let line_height = 25;

            // ROM title
            self.draw_text(surface, &rom.name, info_x, y, Self::PRIMARY_COLOR, 2);
            y += line_height * 2;

            // File info
            let size_mb = rom.file_size as f64 / 1024.0 / 1024.0;
            self.draw_text(
                surface,
                &format!("Size: {:.1} MB", size_mb),
                info_x,
                y,
                Self::CREDITS_COLOR,
                1,
            );
            y += line_height;

            // Battery backup status
            let battery_text = if rom.is_battery_backed {
                "Save Support: Yes"
            } else {
                "Save Support: No"
            };
            let battery_color = if rom.is_battery_backed {
                Self::BATTERY_COLOR
            } else {
                Self::CREDITS_COLOR
            };
            self.draw_text(surface, battery_text, info_x, y, battery_color, 1);
            y += line_height * 2;

            // ROM preview area
            let preview_rect = Rect::new(
                info_x,
                y,
                (self.screen_width - split_x - 40) as u32,
                (self.screen_height - y as u32 - 100).min(200),
            );

            // Try to find and display game image
            let image_found =
                self.try_render_game_image(surface, game, preview_rect, menu_context.debug);

            if !image_found {
                // Only draw gray background if no image found
                surface
                    .fill_rect(preview_rect, Color::RGBA(40, 40, 50, 255))
                    .unwrap();

                // Show placeholder text if no image found
                let preview_text_y = y + preview_rect.height() as i32 / 2;
                self.draw_text_centered(
                    surface,
                    "Game Preview",
                    info_x + preview_rect.width() as i32 / 2,
                    preview_text_y - 10,
                    Self::SECONDARY_COLOR,
                    1,
                );
                self.draw_text_centered(
                    surface,
                    "(No image found)",
                    info_x + preview_rect.width() as i32 / 2,
                    preview_text_y + 10,
                    Self::CREDITS_COLOR,
                    1,
                );
            }
        } else {
            self.draw_text(
                surface,
                "No game selected",
                info_x,
                start_y + 50,
                Self::CREDITS_COLOR,
                2,
            );
        }
    }

    fn clean_name_for_image(name: &str) -> String {
        // Clean the game name to match potential image filenames
        name.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' => c.to_ascii_lowercase(),
                _ => '_',
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }

    fn try_render_game_image(
        surface: &mut Surface,
        game: &GameInfo,
        rect: Rect,
        debug: bool,
    ) -> bool {
        use sdl2::image::LoadSurface;
        use std::fs;
        use std::path::Path;

        // Extract filename from path without extension
        let path = Path::new(&game.path);
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&game.name);

        // Look for images with common extensions
        let extensions = ["png", "jpg", "jpeg", "bmp", "gif"];

        if debug {
            println!("Image Debug: Looking for images for game '{}'", game.name);
            println!("Image Debug: File stem: '{}'", file_stem);
        }

        // Try both original name, cleaned name, and file stem
        let game_name_clean = self.clean_name_for_image(&game.name);
        let file_stem_clean = self.clean_name_for_image(file_stem);
        let names_to_try = vec![&game.name, &game_name_clean, file_stem, &file_stem_clean];

        if debug {
            println!(
                "Image Debug: Original name: '{}', Cleaned name: '{}'",
                game.name, game_name_clean
            );
            println!(
                "Image Debug: File stem: '{}', Cleaned stem: '{}'",
                file_stem, file_stem_clean
            );
        }

        for name in &names_to_try {
            if debug {
                println!("Image Debug: Trying name: '{}'", name);
            }
            for ext in &extensions {
                let image_path = format!("roms/imgs/{}.{}", name, ext);

                if debug {
                    println!("Image Debug: Checking path: {}", image_path);
                }

                if Path::new(&image_path).exists() {
                    if debug {
                        println!("Image Debug: Found image: {}", image_path);
                    }
                    // Try to load the image
                    match Surface::from_file(&image_path) {
                        Ok(image_surface) => {
                            // Calculate scaling to fit the preview area while maintaining aspect ratio
                            let img_width = image_surface.width();
                            let img_height = image_surface.height();
                            let preview_width = rect.width();
                            let preview_height = rect.height();

                            // Calculate scale factor to fit image in preview area
                            let scale_x = preview_width as f32 / img_width as f32;
                            let scale_y = preview_height as f32 / img_height as f32;
                            let scale = scale_x.min(scale_y); // Use smaller scale to maintain aspect ratio

                            let scaled_width = (img_width as f32 * scale) as u32;
                            let scaled_height = (img_height as f32 * scale) as u32;

                            // Center the image in the preview area
                            let dest_x = rect.x + (preview_width as i32 - scaled_width as i32) / 2;
                            let dest_y =
                                rect.y + (preview_height as i32 - scaled_height as i32) / 2;

                            // Create destination rectangle
                            let dest_rect = Rect::new(dest_x, dest_y, scaled_width, scaled_height);

                            // Blit the image to the surface (this will scale automatically)
                            if let Err(_e) = image_surface.blit_scaled(None, surface, dest_rect) {
                                // Fall back to showing text
                                let center_x = rect.x + rect.width() as i32 / 2;
                                let center_y = rect.y + rect.height() as i32 / 2;
                                self.draw_text_centered(
                                    surface,
                                    "Image load error",
                                    center_x,
                                    center_y,
                                    Self::CREDITS_COLOR,
                                    1,
                                );
                            }

                            return true;
                        }
                        Err(_e) => {
                            // Continue to try other formats or names
                        }
                    }
                }
            }
        }

        // If exact match fails, try case-insensitive matching
        if debug {
            println!(
                "Image Debug: Exact match failed, trying case-insensitive matching in roms/imgs/"
            );
        }

        if let Ok(entries) = fs::read_dir("roms/imgs") {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if debug {
                        println!("Image Debug: Found file in directory: {}", file_name);
                    }
                    // Get the file name without extension
                    if let Some(stem) = Path::new(&file_name).file_stem() {
                        if let Some(stem_str) = stem.to_str() {
                            // Check if the stem matches any of our name variants (case-insensitive)
                            let stem_lower = stem_str.to_lowercase();
                            let matches = names_to_try
                                .iter()
                                .any(|name| name.to_lowercase() == stem_lower);

                            if matches {
                                if debug {
                                    println!("Image Debug: Case-insensitive match found: {} matches game", stem_str);
                                }

                                let image_path = format!("roms/imgs/{}", file_name);

                                // Try to load the image
                                match Surface::from_file(&image_path) {
                                    Ok(image_surface) => {
                                        // Calculate scaling to fit the preview area while maintaining aspect ratio
                                        let img_width = image_surface.width();
                                        let img_height = image_surface.height();
                                        let preview_width = rect.width();
                                        let preview_height = rect.height();

                                        // Calculate scale factor to fit image in preview area
                                        let scale_x = preview_width as f32 / img_width as f32;
                                        let scale_y = preview_height as f32 / img_height as f32;
                                        let scale = scale_x.min(scale_y); // Use smaller scale to maintain aspect ratio

                                        let scaled_width = (img_width as f32 * scale) as u32;
                                        let scaled_height = (img_height as f32 * scale) as u32;

                                        // Center the image in the preview area
                                        let dest_x = rect.x
                                            + (preview_width as i32 - scaled_width as i32) / 2;
                                        let dest_y = rect.y
                                            + (preview_height as i32 - scaled_height as i32) / 2;

                                        // Create destination rectangle
                                        let dest_rect =
                                            Rect::new(dest_x, dest_y, scaled_width, scaled_height);

                                        // Blit the image to the surface (this will scale automatically)
                                        if let Err(_e) =
                                            image_surface.blit_scaled(None, surface, dest_rect)
                                        {
                                            // Fall back to showing text
                                            let center_x = rect.x + rect.width() as i32 / 2;
                                            let center_y = rect.y + rect.height() as i32 / 2;
                                            self.draw_text_centered(
                                                surface,
                                                "Image load error",
                                                center_x,
                                                center_y,
                                                Self::CREDITS_COLOR,
                                                1,
                                            );
                                        }

                                        return true;
                                    }
                                    Err(_e) => {
                                        // Continue to try other files
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if debug {
            println!("Image Debug: No image found for game '{}'", game.name);
        }

        false
    }

    fn draw_title_text(&self, surface: &mut Surface, center_x: i32, center_y: i32) {
        // ASCII art style title using standard ASCII characters
        let title_lines = vec![
            "########  ##     ##  ######  ######## ######## ######## ",
            "##     ## ##     ## ##    ##    ##    ##       ##     ##",
            "##     ## ##     ## ##          ##    ##       ##     ##",
            "########  ##     ##  ######     ##    ######   ##     ##",
            "##   ##   ##     ##       ##    ##    ##       ##     ##",
            "##    ##  ##     ## ##    ##    ##    ##       ##     ##",
            "##     ##  #######   ######     ##    ######## ######## ",
            "",
            "########   #######  ##     ##",
            "##     ## ##     ## ###   ###",
            "##     ## ##     ## #### ####",
            "########  ##     ## ## ### ##",
            "##   ##   ##     ## ##     ##",
            "##    ##  ##     ## ##     ##",
            "##     ##  #######  ##     ##",
        ];

        let scale = 1;
        let line_height = 12 * scale;

        let total_height = title_lines.len() as i32 * line_height;
        let start_y = center_y - total_height / 2;

        for (i, line) in title_lines.iter().enumerate() {
            if !line.is_empty() {
                // Center each line individually so "ROM" is centered under "RUSTED"
                self.draw_text_centered(
                    surface,
                    line,
                    center_x,
                    start_y + i as i32 * line_height,
                    Self::PRIMARY_COLOR,
                    scale as u32,
                );
            }
        }
    }

    fn draw_text_centered(
        &self,
        surface: &mut Surface,
        text: &str,
        center_x: i32,
        y: i32,
        color: Color,
        scale: u32,
    ) {
        let char_width = 7 * scale as i32; // Slightly wider for better readability
        let text_width = text.len() as i32 * char_width;
        let x = center_x - text_width / 2;
        self.draw_text(surface, text, x, y, color, scale);
    }

    fn draw_text(&self, surface: &mut Surface, text: &str, x: i32, y: i32, color: Color, scale: u32) {
        let char_width = 7 * scale as i32; // Consistent character width

        for (i, ch) in text.chars().enumerate() {
            let char_x = x + i as i32 * char_width;
            self.draw_char(surface, ch, char_x, y, color, scale);
        }
    }

    fn draw_char(&self, surface: &mut Surface, ch: char, x: i32, y: i32, color: Color, scale: u32) {
        // Character bitmap patterns (5x7 pixel patterns)
        let char_width = 6 * scale;
        let char_height = 8 * scale;
        let pixel_size = scale;

        // Define bitmap patterns for characters (5x7 grid)
        let bitmap = match ch.to_ascii_uppercase() {
            ' ' => vec![], // Space
            '#' => {
                // Solid block for ASCII art
                let rect = Rect::new(x, y, char_width, char_height);
                surface.fill_rect(rect, color).unwrap();
                return;
            }
            'A' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
            ],
            'B' => vec![
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
            ],
            'C' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            'D' => vec![
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
            ],
            'E' => vec![
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
            ],
            'F' => vec![
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
            ],
            'G' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            'H' => vec![
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
            ],
            'I' => vec![
                [0, 1, 1, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 1, 1, 0],
            ],
            'L' => vec![
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
            ],
            'M' => vec![
                [1, 0, 0, 0, 1],
                [1, 1, 0, 1, 1],
                [1, 0, 1, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
            ],
            'N' => vec![
                [1, 0, 0, 0, 1],
                [1, 1, 0, 0, 1],
                [1, 0, 1, 0, 1],
                [1, 0, 0, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
            ],
            'O' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            'P' => vec![
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
            ],
            'R' => vec![
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
                [1, 0, 1, 0, 0],
                [1, 0, 0, 1, 0],
                [1, 0, 0, 0, 1],
            ],
            'S' => vec![
                [0, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [0, 1, 1, 1, 0],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
            ],
            'T' => vec![
                [1, 1, 1, 1, 1],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            'U' => vec![
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            'Y' => vec![
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            'W' => vec![
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 1, 0, 1],
                [1, 0, 1, 0, 1],
                [1, 1, 0, 1, 1],
                [1, 0, 0, 0, 1],
            ],
            'V' => vec![
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 0, 1, 0],
                [0, 1, 0, 1, 0],
                [0, 0, 1, 0, 0],
            ],
            'K' => vec![
                [1, 0, 0, 0, 1],
                [1, 0, 0, 1, 0],
                [1, 0, 1, 0, 0],
                [1, 1, 0, 0, 0],
                [1, 0, 1, 0, 0],
                [1, 0, 0, 1, 0],
                [1, 0, 0, 0, 1],
            ],
            'J' => vec![
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            'Q' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 1, 0, 1],
                [1, 0, 0, 1, 0],
                [0, 1, 1, 0, 1],
            ],
            'X' => vec![
                [1, 0, 0, 0, 1],
                [0, 1, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 1, 0],
                [1, 0, 0, 0, 1],
            ],
            'Z' => vec![
                [1, 1, 1, 1, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
            ],
            '0' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 1, 1],
                [1, 0, 1, 0, 1],
                [1, 1, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            '1' => vec![
                [0, 0, 1, 0, 0],
                [0, 1, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 1, 1, 0],
            ],
            '2' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [1, 1, 1, 1, 1],
            ],
            '3' => vec![
                [1, 1, 1, 1, 0],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
            ],
            '4' => vec![
                [1, 0, 0, 1, 0],
                [1, 0, 0, 1, 0],
                [1, 0, 0, 1, 0],
                [1, 1, 1, 1, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 1, 0],
            ],
            '5' => vec![
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 0],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
            ],
            '6' => vec![
                [0, 1, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            '7' => vec![
                [1, 1, 1, 1, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
            ],
            '8' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
            ],
            '9' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 1],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 1],
                [1, 1, 1, 1, 0],
            ],
            '&' => vec![
                [0, 1, 1, 0, 0],
                [1, 0, 0, 1, 0],
                [1, 0, 0, 1, 0],
                [0, 1, 1, 0, 0],
                [1, 0, 1, 0, 1],
                [1, 0, 0, 1, 0],
                [0, 1, 1, 0, 1],
            ],
            '>' => vec![
                [1, 0, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [1, 0, 0, 0, 0],
            ],
            '<' => vec![
                [0, 0, 0, 0, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 0, 1],
            ],
            ':' => vec![
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '.' => vec![
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '!' => vec![
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            '?' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            '|' => vec![
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            '(' => vec![
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            ')' => vec![
                [0, 0, 1, 0, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
            ],
            '-' => vec![
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '▶' => vec![
                [1, 0, 0, 0, 0],
                [1, 1, 0, 0, 0],
                [1, 1, 1, 0, 0],
                [1, 1, 1, 1, 0],
                [1, 1, 1, 0, 0],
                [1, 1, 0, 0, 0],
                [1, 0, 0, 0, 0],
            ],
            '↑' => vec![
                [0, 0, 1, 0, 0],
                [0, 1, 1, 1, 0],
                [1, 0, 1, 0, 1],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
            ],
            '↓' => vec![
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 1, 0, 0],
                [1, 0, 1, 0, 1],
                [0, 1, 1, 1, 0],
                [0, 0, 1, 0, 0],
            ],
            '@' => vec![
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 1, 1, 1],
                [1, 0, 1, 0, 1],
                [1, 0, 1, 1, 1],
                [1, 0, 0, 0, 0],
                [0, 1, 1, 1, 1],
            ],
            '"' => vec![
                [0, 1, 0, 1, 0],
                [0, 1, 0, 1, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '\'' => vec![
                [0, 1, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '\\' => vec![
                [1, 0, 0, 0, 0],
                [0, 1, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 0, 1],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '/' => vec![
                [0, 0, 0, 0, 1],
                [0, 0, 0, 1, 0],
                [0, 0, 1, 0, 0],
                [0, 1, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '`' => vec![
                [0, 1, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '_' => vec![
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
            ],
            'o' => vec![
                [0, 0, 0, 0, 0],
                [0, 1, 1, 1, 0],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [0, 1, 1, 1, 0],
                [0, 0, 0, 0, 0],
            ],
            '=' => vec![
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
                [0, 0, 0, 0, 0],
                [1, 1, 1, 1, 1],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            '^' => vec![
                [0, 0, 1, 0, 0],
                [0, 1, 0, 1, 0],
                [1, 0, 0, 0, 1],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            _ => vec![
                [1, 1, 1, 1, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 0, 0, 0, 1],
                [1, 1, 1, 1, 1],
            ], // Default rectangle for unknown chars
        };

        // Draw the bitmap pattern
        for (row, line) in bitmap.iter().enumerate() {
            for (col, &pixel) in line.iter().enumerate() {
                if pixel == 1 {
                    let pixel_x = x + col as i32 * pixel_size as i32;
                    let pixel_y = y + row as i32 * pixel_size as i32;
                    let rect = Rect::new(pixel_x, pixel_y, pixel_size, pixel_size);
                    surface.fill_rect(rect, color).unwrap();
                }
            }
        }
    }

    fn render_palette_selection(
        &self,
        surface: &mut Surface,
    ) {
        let center_x = self.screen_width as i32 / 2;

        // Draw title
        self.draw_text_centered(
            surface,
            "SELECT COLOR PALETTE",
            center_x,
            25,
            Self::PRIMARY_COLOR,
            2,
        );

        let start_y = 60;
        let line_height = 50; // Reduced from 60
        let preview_size = 28; // Reduced from 40
        let preview_spacing = 3; // Reduced spacing between color boxes

        for (i, palette) in menu_context.available_palettes.iter().enumerate() {
            let y = start_y + (i as i32 * line_height);
            let is_selected = i == menu_context.selected_palette_index;
            let is_current = palette == menu_context.get_current_palette();

            // Draw selection highlight with reduced width
            if is_selected {
                let highlight_rect =
                    Rect::new(10, y - 3, screen_width - 20, line_height as u32 - 6);
                surface
                    .fill_rect(highlight_rect, Color::RGBA(100, 200, 255, 30))
                    .unwrap();
            }

            // Draw selection arrow
            if is_selected {
                self.draw_text(surface, ">", 15, y + 12, Self::SELECTED_COLOR, 2);
            }

            // Draw palette name with shortened versions
            let name_color = if is_selected {
                Self::SELECTED_COLOR
            } else if is_current {
                Self::BATTERY_COLOR // Use green to indicate current
            } else {
                Self::PRIMARY_COLOR
            };

            // Use shorter names to fit better
            let short_name = match palette {
                crate::menu::ColorPalette::ClassicGameBoy => "CLASSIC GAME BOY",
                crate::menu::ColorPalette::GreenScale => "GREENSCALE",
                crate::menu::ColorPalette::PurpleShades => "PURPLE DREAMS",
                crate::menu::ColorPalette::BlueShades => "OCEAN BLUE",
                crate::menu::ColorPalette::Sepia => "VINTAGE SEPIA",
                crate::menu::ColorPalette::RedShades => "RUBY RED",
                crate::menu::ColorPalette::CyberpunkGreen => "CYBERPUNK",
                crate::menu::ColorPalette::Ocean => "DEEP OCEAN",
            };

            let palette_name = if is_current {
                format!("{}", short_name)
            } else {
                short_name.to_string()
            };

            self.draw_text(surface, &palette_name, 35, y + 12, name_color, 1); // Reduced scale from 2 to 1

            // Draw color preview boxes - positioned on the right side
            let colors = palette.get_colors();
            let total_preview_width = (preview_size + preview_spacing) * 4 - preview_spacing;
            let box_start_x = screen_width as i32 - total_preview_width - 15; // 15px margin from right

            for (j, &color) in colors.iter().enumerate() {
                let box_x = box_start_x + (j as i32 * (preview_size + preview_spacing));
                let box_rect = Rect::new(box_x, y + 5, preview_size as u32, preview_size as u32);

                // Convert ARGB to RGB for SDL2
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;
                let sdl_color = Color::RGB(r, g, b);

                surface.fill_rect(box_rect, sdl_color).unwrap();

                // Draw border with thinner lines
                let border_color = if is_selected {
                    Self::SELECTED_COLOR
                } else {
                    Color::RGB(100, 100, 100)
                };
                self.draw_rect_border(surface, box_rect, border_color);
            }
        }

        // Draw instructions
        let instructions_y = screen_height as i32 - 45;
        self.draw_text_centered(
            surface,
            "UP/DOWN: NAVIGATE | ENTER: SELECT | BACKSPACE: BACK",
            center_x,
            instructions_y,
            Self::SECONDARY_COLOR,
            1,
        );
    }

    fn draw_rect_border(surface: &mut Surface, rect: Rect, color: Color) {
        // Draw border lines manually since SDL2 doesn't have a direct border function
        // Top line
        surface
            .fill_rect(Rect::new(rect.x(), rect.y(), rect.width(), 1), color)
            .unwrap();
        // Bottom line
        surface
            .fill_rect(
                Rect::new(
                    rect.x(),
                    rect.y() + rect.height() as i32 - 1,
                    rect.width(),
                    1,
                ),
                color,
            )
            .unwrap();
        // Left line
        surface
            .fill_rect(Rect::new(rect.x(), rect.y(), 1, rect.height()), color)
            .unwrap();
        // Right line
        surface
            .fill_rect(
                Rect::new(
                    rect.x() + rect.width() as i32 - 1,
                    rect.y(),
                    1,
                    rect.height(),
                ),
                color,
            )
            .unwrap();
    }
}
