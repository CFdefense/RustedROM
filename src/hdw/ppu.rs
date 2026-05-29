use crate::hdw::interrupts::Interrupts;
/**
 * PPU Module - Game Boy Picture Processing Unit
 *
 * This module implements the Game Boy's Picture Processing Unit (PPU), which is responsible
 * for generating the video output displayed on the LCD screen. The PPU operates in parallel
 * with the CPU and manages graphics rendering, sprite display, and LCD timing.
 *
 * Core Components:
 * - Video RAM (VRAM): 8KB for tile data and tile maps
 * - Object Attribute Memory (OAM): 160 bytes for sprite attributes  
 * - LCD Controller: Manages display timing and rendering modes
 * - Pixel Pipeline: Fetches and processes background/window/sprite pixels
 * - DMA Controller: Transfers sprite data during specific timing windows
 *
 * Rendering Pipeline:
 * 1. OAM Scan: Search for sprites visible on current scanline
 * 2. Pixel Transfer: Fetch background, window, and sprite data
 * 3. HBlank: Horizontal blanking period between scanlines
 * 4. VBlank: Vertical blanking period after frame completion
 *
 * LCD Modes and Timing:
 * - Mode 0 (HBlank): 204 cycles - CPU can access VRAM/OAM
 * - Mode 1 (VBlank): 4560 cycles - CPU can access VRAM/OAM  
 * - Mode 2 (OAM): 80 cycles - CPU cannot access OAM
 * - Mode 3 (Transfer): 172 cycles - CPU cannot access VRAM/OAM
 *
 * Graphics Features:
 * - 160x144 pixel display with 4-color grayscale palette
 * - 40 hardware sprites (8x8 or 8x16 pixels) with priority system
 * - Background and window layers with scrolling support
 * - Hardware-accelerated pixel FIFO for authentic timing
 *
 * The PPU achieves cycle-accurate timing to ensure proper game compatibility
 * and authentic visual behavior matching original Game Boy hardware.
 */
use crate::hdw::lcd::{LcdMode, StatSrc, LCD};
use crate::hdw::ppu_pipeline::{FIFOState, PixelFIFO};
use crate::hdw::ui::{delay, get_ticks};

/**
 * OAMEntry - Object Attribute Memory Entry
 *
 * Represents a single sprite's attributes stored in OAM.
 * Each sprite uses 4 bytes in OAM memory (40 sprites total).
 */
#[derive(Copy, Clone)]
pub struct OAMEntry {
    /// Y position on screen (offset by 16 pixels)
    pub y: u8,
    /// X position on screen (offset by 8 pixels)  
    pub x: u8,
    /// Tile number in VRAM tile data area
    pub tile: u8,
    /// Attribute flags (palette, priority, flip bits)
    pub flags: u8,
}

// Display timing constants matching Game Boy hardware specifications
const LINES_PER_FRAME: u8 = 154; // Total scanlines including VBlank
const TICKS_PER_LINE: u32 = 456; // CPU cycles per scanline
const YRES: u8 = 144; // Visible scanlines
const XRES: u8 = 160; // Pixels per scanline

/**
 * OAMLineEntry - Linked List Node for Sprite Processing
 *
 * Used to build sorted lists of sprites visible on current scanline.
 * Sprites are sorted by X position for proper priority handling.
 */
pub struct OAMLineEntry {
    /// Sprite attributes for this entry
    pub entry: OAMEntry,
    /// Pointer to next sprite in sorted list
    pub next: Option<Box<OAMLineEntry>>,
}

impl OAMLineEntry {
    pub fn new(entry: OAMEntry) -> Self {
        OAMLineEntry { entry, next: None }
    }
}

impl OAMEntry {
    pub fn new() -> Self {
        OAMEntry {
            y: 0,
            x: 0,
            tile: 0,
            flags: 0,
        }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [self.y, self.x, self.tile, self.flags]
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        OAMEntry {
            y: bytes[0],
            x: bytes[1],
            tile: bytes[2],
            flags: bytes[3],
        }
    }
}

/**
 * PPU - Picture Processing Unit Controller
 *
 * Main PPU controller that manages all graphics rendering operations.
 * Coordinates between LCD timing, pixel pipeline, sprite processing,
 * and frame generation to produce authentic Game Boy video output.
 *
 * The PPU operates in four distinct modes during each frame:
 * - OAM Scan: Find sprites for current scanline
 * - Pixel Transfer: Render pixels using background, window, and sprites  
 * - HBlank: Horizontal blanking between scanlines
 * - VBlank: Vertical blanking between frames
 */
pub struct PPU {
    pub oam_ram: [OAMEntry; 40],
    pub vram: [u8; 0x2000],
    pub ly: u8,                 // Current scanline
    pub current_frame: u32,     // Current frame number
    pub video_buffer: Vec<u32>, // Video buffer for frame (YRES * XRES * 32-bit pixels)
    pub line_ticks: u32,        // Ticks for current scanline
    pub lcd: LCD,               // LCD controller
    pub pixel_fifo: PixelFIFO,

    // Sprite/fifo info
    pub line_sprite_count: u8,                   // 0 - 10 sprites per line
    pub line_sprites: Option<Box<OAMLineEntry>>, // linked list of current line sprites
    pub line_entry_array: [OAMLineEntry; 10],
    pub fetched_entry_count: u8,
    pub fetched_entries: [OAMEntry; 3],

    // Window info
    pub window_line: u8,

    // Frame timing
    target_frame_time: u32,
    prev_frame_time: u64,
    start_timer: u64,
    frame_count: u32,
    pub current_fps: u32,
}

impl PPU {
    pub fn new() -> Self {
        let mut ppu = PPU {
            oam_ram: [OAMEntry::new(); 40],
            vram: [0; 0x2000],
            ly: 0,
            line_ticks: 0,
            current_frame: 0,
            video_buffer: vec![0; (YRES as usize) * (XRES as usize)], // Allocate frame buffer
            lcd: LCD::new(),
            pixel_fifo: PixelFIFO::new(),

            // Sprite/fifo info
            line_sprite_count: 0,
            line_sprites: None,
            line_entry_array: std::array::from_fn(|_| OAMLineEntry::new(OAMEntry::new())),
            fetched_entry_count: 0,
            fetched_entries: [OAMEntry::new(); 3],

            // Window info
            window_line: 0,

            // Frame timing (60 FPS)
            target_frame_time: 1000 / 60,
            prev_frame_time: 0,
            start_timer: 0,
            frame_count: 0,
            current_fps: 0,
        };

        // Set initial LCD mode to OAM
        ppu.lcd.lcds_mode_set(LcdMode::OAM);

        ppu
    }

    pub fn pipeline_process(&mut self) {
        // Instead of using unsafe code, manually inline the pipeline operations
        self.pixel_fifo.map_y = self.lcd.ly.wrapping_add(self.lcd.scy);
        self.pixel_fifo.map_x = self.pixel_fifo.fetch_x.wrapping_add(self.lcd.scx);

        // Calculate tile_y - use window relative position if window is active
        if self.window_visible() && self.lcd.ly >= self.lcd.wy {
            let window_x = self.lcd.wx;
            if self.pixel_fifo.fetch_x + 7 >= window_x {
                // Use window-relative tile_y
                let window_relative_y = self.lcd.ly.saturating_sub(self.lcd.wy);
                self.pixel_fifo.tile_y = ((window_relative_y) % 8) * 2;
            } else {
                // Use normal background tile_y
                self.pixel_fifo.tile_y = ((self.lcd.ly.wrapping_add(self.lcd.scy)) % 8) * 2;
            }
        } else {
            // Use normal background tile_y
            self.pixel_fifo.tile_y = ((self.lcd.ly.wrapping_add(self.lcd.scy)) % 8) * 2;
        }

        if (self.line_ticks & 1) == 0 {
            // Even Line
            self.pipeline_fetch();
        }

        self.pipeline_push_pixel();
    }

    fn pipeline_fetch(&mut self) {
        match self.pixel_fifo.state {
            FIFOState::TILE => {
                self.fetched_entry_count = 0;
                self.check_window_state(); // Check window state before fetching tiles

                if self.lcd.lcdc_bgw_enable() {
                    // First load background tile
                    let map_address = self.lcd.lcdc_bg_map_area()
                        + ((self.pixel_fifo.map_x / 8) as u16)
                        + (((self.pixel_fifo.map_y / 8) as u16) * 32);

                    self.pixel_fifo.bgw_fetch_data[0] = self.read_vram(map_address);

                    if self.lcd.lcdc_bgw_data_area() == 0x8800 {
                        self.pixel_fifo.bgw_fetch_data[0] =
                            self.pixel_fifo.bgw_fetch_data[0].wrapping_add(128);
                    }

                    // Check if window should override background
                    if self.window_visible() && self.lcd.ly >= self.lcd.wy {
                        let window_x = self.lcd.wx;
                        if self.pixel_fifo.fetch_x + 7 >= window_x {
                            self.pipeline_load_window_tile();
                        }
                    }
                }

                if self.lcd.lcdc_obj_enable() && self.line_sprites.is_some() {
                    self.pipeline_load_sprite_tile();
                }

                self.pixel_fifo.state = FIFOState::DATA0;
                self.pixel_fifo.fetch_x = self.pixel_fifo.fetch_x.wrapping_add(8);
            }
            FIFOState::DATA0 => {
                let data_address = self.lcd.lcdc_bgw_data_area()
                    + ((self.pixel_fifo.bgw_fetch_data[0] as u16) * 16)
                    + (self.pixel_fifo.tile_y as u16);

                self.pixel_fifo.bgw_fetch_data[1] = self.read_vram(data_address);
                self.pipeline_load_sprite_data(0);
                self.pixel_fifo.state = FIFOState::DATA1;
            }
            FIFOState::DATA1 => {
                let data_address = self.lcd.lcdc_bgw_data_area()
                    + ((self.pixel_fifo.bgw_fetch_data[0] as u16) * 16)
                    + (self.pixel_fifo.tile_y as u16 + 1);

                self.pixel_fifo.bgw_fetch_data[2] = self.read_vram(data_address);
                self.pipeline_load_sprite_data(1);
                self.pixel_fifo.state = FIFOState::IDLE;
            }
            FIFOState::IDLE => {
                self.pixel_fifo.state = FIFOState::PUSH;
            }
            FIFOState::PUSH => {
                if self.pipeline_add() {
                    self.pixel_fifo.state = FIFOState::TILE;
                }
            }
        }
    }

    fn read_vram(&self, address: u16) -> u8 {
        if address >= 0x8000 && address <= 0x9FFF {
            self.vram[(address - 0x8000) as usize]
        } else {
            0xFF
        }
    }

    fn increment_ly(&mut self) -> Vec<Interrupts> {
        let mut interrupts = Vec::new();

        // Only increment window line counter when we're actually drawing window content
        if self.window_visible() && self.lcd.ly >= self.lcd.wy && self.lcd.wx <= 166 {
            // Check if we actually rendered any window pixels on this line
            let window_x = self.lcd.wx.saturating_sub(7);
            if window_x < XRES {
                self.window_line += 1;
            }
        }

        self.ly += 1;

        if self.ly == self.lcd.lyc {
            self.lcd.lcds_lyc_set(true);

            if self.lcd.lcds_stat_int(StatSrc::LYC) {
                interrupts.push(Interrupts::LCDSTAT);
            }
        } else {
            self.lcd.lcds_lyc_set(false);
        }

        interrupts
    }

    fn ppu_mode_oam(&mut self) -> Vec<Interrupts> {
        if self.line_ticks >= 80 {
            self.lcd.lcds_mode_set(LcdMode::Transfer);

            self.pixel_fifo.state = FIFOState::TILE;
            self.pixel_fifo.line_x = 0;
            self.pixel_fifo.fetch_x = 0;
            self.pixel_fifo.pushed_x = 0;
            self.pixel_fifo.fifo_x = 0;
        }

        if self.line_ticks == 1 {
            // Read OAM on first tick
            self.line_sprites = None;
            self.line_sprite_count = 0;

            self.load_line_sprites();
        }

        Vec::new()
    }

    fn ppu_mode_xfer(&mut self) -> Vec<Interrupts> {
        // Now we can enable pipeline processing since it doesn't need bus access
        self.pipeline_process();
        let mut interrupts = Vec::new();

        if self.pixel_fifo.pushed_x >= XRES {
            self.pixel_fifo.pipeline_fifo_reset();
            self.lcd.lcds_mode_set(LcdMode::HBlank);

            if self.lcd.lcds_stat_int(StatSrc::HBlank) {
                interrupts.push(Interrupts::LCDSTAT);
            }
        }
        interrupts
    }

    fn ppu_mode_vblank(&mut self) -> Vec<Interrupts> {
        let mut interrupts = Vec::new();

        if self.line_ticks >= TICKS_PER_LINE {
            interrupts.extend(self.increment_ly());

            if self.ly >= LINES_PER_FRAME {
                self.lcd.lcds_mode_set(LcdMode::OAM);
                self.ly = 0;
                self.window_line = 0;
            }

            self.line_ticks = 0;
        }

        interrupts
    }

    fn ppu_mode_hblank(&mut self, cart: &mut crate::hdw::cart::Cartridge) -> Vec<Interrupts> {
        let mut interrupts = Vec::new();

        if self.line_ticks >= TICKS_PER_LINE {
            interrupts.extend(self.increment_ly());

            if self.ly >= YRES {
                self.lcd.lcds_mode_set(LcdMode::VBlank);

                interrupts.push(Interrupts::VBLANK);

                if self.lcd.lcds_stat_int(StatSrc::VBlank) {
                    interrupts.push(Interrupts::LCDSTAT);
                }

                self.current_frame += 1;

                // Calculate FPS
                let end = get_ticks();
                let frame_time = end - self.prev_frame_time;

                if frame_time < self.target_frame_time as u64 {
                    delay((self.target_frame_time as u64 - frame_time) as u32);
                }

                if end - self.start_timer >= 1000 {
                    self.current_fps = self.frame_count;
                    self.start_timer = end;
                    self.frame_count = 0;

                    // Save Cart Battery if needed
                    if cart.cart_needs_save() {
                        cart.cart_save_battery();
                    }
                }

                self.frame_count += 1;
                self.prev_frame_time = get_ticks();
            } else {
                self.lcd.lcds_mode_set(LcdMode::OAM);
            }

            self.line_ticks = 0;
        }

        interrupts
    }

    pub fn ppu_tick(&mut self, cart: &mut crate::hdw::cart::Cartridge) -> Vec<Interrupts> {
        self.line_ticks += 1;

        match self.lcd.lcds_mode() {
            LcdMode::OAM => self.ppu_mode_oam(),
            LcdMode::Transfer => self.ppu_mode_xfer(),
            LcdMode::VBlank => self.ppu_mode_vblank(),
            LcdMode::HBlank => self.ppu_mode_hblank(cart),
        }
    }

    pub fn update_lcd_ly(&mut self) {
        self.lcd.ly = self.ly;
    }

    pub fn ppu_oam_write(&mut self, mut address: u16, value: u8) {
        if address >= 0xFE00 {
            address -= 0xFE00;
        }
        let entry_index = (address / 4) as usize;
        let byte_index = (address % 4) as usize;
        let mut entry_bytes = self.oam_ram[entry_index].to_bytes();
        entry_bytes[byte_index] = value;
        self.oam_ram[entry_index] = OAMEntry::from_bytes(entry_bytes);
    }

    pub fn ppu_oam_read(&self, mut address: u16) -> u8 {
        if address >= 0xFE00 {
            address -= 0xFE00;
        }
        let entry_index = (address / 4) as usize;
        let byte_index = (address % 4) as usize;
        self.oam_ram[entry_index].to_bytes()[byte_index]
    }

    pub fn ppu_vram_write(&mut self, address: u16, value: u8) {
        self.vram[(address - 0x8000) as usize] = value;
    }

    pub fn ppu_vram_read(&self, address: u16) -> u8 {
        self.vram[(address - 0x8000) as usize]
    }

    pub fn load_line_sprites(&mut self) {
        let cur_y = self.lcd.ly as i16;
        let sprite_height = self.lcd.lcdc_obj_height() as i16;

        // Clear line entry array
        self.line_entry_array = std::array::from_fn(|_| OAMLineEntry::new(OAMEntry::new()));

        for i in 0..40 {
            let entry: OAMEntry = self.oam_ram[i];

            if entry.x == 0 {
                continue;
            }

            // max 10 sprites allowed per line
            if self.line_sprite_count >= 10 {
                break;
            }

            // Check if sprite is on current line (Game Boy sprites have Y offset of 16)
            if entry.y <= cur_y as u8 + 16 && entry.y + sprite_height as u8 > cur_y as u8 + 16 {
                let entry_index = self.line_sprite_count as usize;
                self.line_entry_array[entry_index] = OAMLineEntry::new(entry);
                self.line_sprite_count += 1;

                // Insert into sorted linked list by x position
                if self.line_sprites.is_none()
                    || self.line_sprites.as_ref().unwrap().entry.x > entry.x
                {
                    let mut new_entry = Box::new(OAMLineEntry::new(entry));
                    new_entry.next = self.line_sprites.take();
                    self.line_sprites = Some(new_entry);
                    continue;
                }

                // Find insertion point in sorted list
                if let Some(ref mut head) = self.line_sprites {
                    let mut current = head;

                    loop {
                        let should_insert_after_current = if let Some(ref next_node) = current.next
                        {
                            next_node.entry.x > entry.x
                        } else {
                            true
                        };

                        if should_insert_after_current {
                            if current.next.is_some() {
                                let mut new_entry = Box::new(OAMLineEntry::new(entry));
                                new_entry.next = current.next.take();
                                current.next = Some(new_entry);
                            } else {
                                current.next = Some(Box::new(OAMLineEntry::new(entry)));
                            }
                            break;
                        }

                        if let Some(ref mut next) = current.next {
                            current = next;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn pipeline_add(&mut self) -> bool {
        if self.pixel_fifo.fifo_size() > 8 {
            return false;
        }

        let x: i16 = self.pixel_fifo.fetch_x as i16 - (8 - (self.lcd.scx % 8)) as i16;

        for i in 0..8 {
            let bit = 7 - i;
            let hi = if (self.pixel_fifo.bgw_fetch_data[1] & (1 << bit)) != 0 {
                1
            } else {
                0
            };
            let lo = if (self.pixel_fifo.bgw_fetch_data[2] & (1 << bit)) != 0 {
                2
            } else {
                0
            };

            let mut color_index = hi | lo;
            let mut color: u32 = self.lcd.bg_colors[color_index as usize];

            if !self.lcd.lcdc_bgw_enable() {
                color = self.lcd.bg_colors[0];
                color_index = 0; // Important: when background is disabled, treat it as transparent (color index 0)
            }

            if self.lcd.lcdc_obj_enable() {
                color = self.fetch_sprite_pixels(bit, color, color_index);
            }

            if (x + i as i16) >= 0 {
                self.pixel_fifo.pixel_fifo_push(color);
                self.pixel_fifo.fifo_x += 1;
            }
        }
        true
    }

    fn pipeline_push_pixel(&mut self) {
        if self.pixel_fifo.fifo_size() > 0 {
            let pixel_data = self.pixel_fifo.pixel_fifo_pop().unwrap();

            if self.pixel_fifo.line_x >= self.lcd.scx % 8 {
                let x = self.pixel_fifo.pushed_x as usize;
                let y = self.lcd.ly as usize;
                let buffer_index = x + (y * XRES as usize);

                if x < XRES as usize && y < YRES as usize && buffer_index < self.video_buffer.len()
                {
                    self.video_buffer[buffer_index] = pixel_data;
                }
                self.pixel_fifo.pushed_x += 1;
            }
            self.pixel_fifo.line_x += 1;
        }
    }

    fn pipeline_load_sprite_tile(&mut self) {
        let mut current_sprite = self.line_sprites.as_ref();

        while let Some(le) = current_sprite {
            let sp_x = (le.entry.x as i16 - 8) + (self.lcd.scx % 8) as i16;

            if (sp_x >= self.pixel_fifo.fetch_x as i16 && sp_x < self.pixel_fifo.fetch_x as i16 + 8)
                || ((sp_x + 8) >= self.pixel_fifo.fetch_x as i16
                    && (sp_x + 8) < self.pixel_fifo.fetch_x as i16 + 8)
            {
                if (self.fetched_entry_count as usize) < 3 {
                    self.fetched_entries[self.fetched_entry_count as usize] = le.entry;
                    self.fetched_entry_count += 1;
                }
            }

            current_sprite = le.next.as_ref();

            if current_sprite.is_none() || self.fetched_entry_count >= 3 {
                break;
            }
        }
    }

    fn pipeline_load_sprite_data(&mut self, offset: u8) {
        let cur_y = self.lcd.ly as i16;
        let sprite_height = self.lcd.lcdc_obj_height();

        for i in 0..self.fetched_entry_count as usize {
            if i >= 3 {
                break;
            }

            let mut ty = ((cur_y + 16 - self.fetched_entries[i].y as i16) * 2) as u8;

            let f_y_flip = (self.fetched_entries[i].flags & (1 << 6)) != 0;
            if f_y_flip {
                ty = ((sprite_height * 2) - 2) - ty;
            }

            let mut tile_index = self.fetched_entries[i].tile;
            if sprite_height == 16 {
                tile_index &= !1;
            }

            let address = 0x8000 + (tile_index as u16 * 16) + ty as u16 + offset as u16;
            self.pixel_fifo.fetch_entry_data[(i * 2) + offset as usize] = self.read_vram(address);
        }
    }

    fn fetch_sprite_pixels(&self, _bit: u8, color: u32, bg_color: u8) -> u32 {
        let mut result_color = color;

        for i in 0..self.fetched_entry_count as usize {
            if i >= 3 {
                break;
            }

            let sprite = &self.fetched_entries[i];
            let sp_x = (sprite.x as i16 - 8) + (self.lcd.scx % 8) as i16;

            if sp_x + 8 < self.pixel_fifo.fifo_x as i16 {
                continue;
            }

            let offset = (self.pixel_fifo.fifo_x as i16) - sp_x;

            if offset < 0 || offset > 7 {
                continue;
            }

            let mut bit = 7 - offset;

            let f_x_flip = (sprite.flags & (1 << 5)) != 0;
            if f_x_flip {
                bit = offset;
            }

            let hi = if (self.pixel_fifo.fetch_entry_data[i * 2] & (1 << bit)) != 0 {
                1
            } else {
                0
            };
            let lo = if (self.pixel_fifo.fetch_entry_data[(i * 2) + 1] & (1 << bit)) != 0 {
                2
            } else {
                0
            };

            let bg_priority = (sprite.flags & (1 << 7)) != 0;
            let sprite_color_index = hi | lo;

            if sprite_color_index == 0 {
                continue; // Transparent sprite pixel
            }

            if !bg_priority || bg_color == 0 {
                let f_pn = (sprite.flags & (1 << 4)) != 0;

                result_color = if f_pn {
                    self.lcd.sp2_colors[sprite_color_index as usize]
                } else {
                    self.lcd.sp1_colors[sprite_color_index as usize]
                };

                if sprite_color_index != 0 {
                    break; // Stop processing more sprites once we find a visible one
                }
            }
        }

        result_color
    }

    pub fn pipeline_load_window_tile(&mut self) {
        if !self.window_visible() {
            return;
        }

        let window_x = self.lcd.wx.saturating_sub(7); // WX=7 means window starts at x=0

        // Calculate window tile coordinates
        let win_tile_x = ((self.pixel_fifo.fetch_x + 7).saturating_sub(window_x)) / 8;

        // Use window_line instead of calculating from LY
        let win_tile_y = self.window_line / 8;

        // Ensure we're within bounds
        if win_tile_x < 32 && win_tile_y < 32 {
            // Get the tile index from the window map
            let map_address =
                self.lcd.lcdc_win_map_area() + (win_tile_x as u16) + (win_tile_y as u16 * 32);

            self.pixel_fifo.bgw_fetch_data[0] = self.read_vram(map_address);

            if self.lcd.lcdc_bgw_data_area() == 0x8800 {
                self.pixel_fifo.bgw_fetch_data[0] =
                    self.pixel_fifo.bgw_fetch_data[0].wrapping_add(128);
            }
        }
    }

    pub fn window_visible(&self) -> bool {
        self.lcd.lcdc_win_enable() && self.lcd.wy < YRES
    }

    pub fn check_window_state(&mut self) {
        // Reset window line counter at the start of each frame
        if self.ly == 0 {
            self.window_line = 0;
        }
    }
}
