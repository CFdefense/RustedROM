// hdw/ppu.rs
// Game Boy Picture Processing Unit
//
// This module implements the Game Boy's Picture Processing Unit (PPU), which is responsible
// for generating the video output displayed on the LCD screen. The PPU operates in parallel
// with the CPU and manages graphics rendering, sprite display, and LCD timing.
//
// # Core Components
//
// - Video RAM (VRAM): 8KB for tile data and tile maps
// - Object Attribute Memory (OAM): 160 bytes for sprite attributes
// - LCD Controller: Manages display timing and rendering modes
// - Pixel Pipeline: Fetches and processes background/window/sprite pixels
// - DMA Controller: Transfers sprite data during specific timing windows
//
// # Rendering Pipeline
//
// 1. OAM Scan: Search for sprites visible on current scanline
// 2. Pixel Transfer: Fetch background, window, and sprite data
// 3. HBlank: Horizontal blanking period between scanlines
// 4. VBlank: Vertical blanking period after frame completion
//
// # LCD Modes and Timing
//
// - Mode 0 (HBlank): 204 cycles - CPU can access VRAM/OAM
// - Mode 1 (VBlank): 4560 cycles - CPU can access VRAM/OAM
// - Mode 2 (OAM): 80 cycles - CPU cannot access OAM
// - Mode 3 (Transfer): 172 cycles - CPU cannot access VRAM/OAM
//
// # Graphics Features
//
// - 160x144 pixel display with 4-color grayscale palette
// - 40 hardware sprites (8x8 or 8x16 pixels) with priority system
// - Background and window layers with scrolling support
// - Hardware-accelerated pixel FIFO for authentic timing
//
// The PPU achieves cycle-accurate timing to ensure proper game compatibility
// and authentic visual behavior matching original Game Boy hardware.

use crate::hdw::interrupts::Interrupts;
use crate::hdw::lcd::{LcdMode, StatSrc, LCD};
use crate::hdw::ppu_pipeline::{FIFOState, PixelFIFO};
use crate::hdw::ui::{delay, get_ticks};

/// Object Attribute Memory entry representing a single sprite.
///
/// Each sprite uses 4 bytes in OAM memory (40 sprites total, 160 bytes).
/// Sprites are positioned with offsets: Y offset by 16, X offset by 8.
///
/// # Attribute Flags (byte 3)
///
/// - Bit 7: BG and Window over OBJ (0=OBJ above, 1=OBJ behind colors 1-3)
/// - Bit 6: Y flip (0=normal, 1=vertically mirrored)
/// - Bit 5: X flip (0=normal, 1=horizontally mirrored)
/// - Bit 4: Palette number (0=OBP0, 1=OBP1)
/// - Bits 3-0: Not used in DMG
#[derive(Copy, Clone)]
pub struct OAMEntry {
    /// Y position on screen (offset by 16 pixels).
    ///
    /// Y=0 means sprite is off-screen above. Y=16 means sprite top is at screen top.
    pub y: u8,

    /// X position on screen (offset by 8 pixels).
    ///
    /// X=0 means sprite is off-screen left. X=8 means sprite left edge is at screen left.
    pub x: u8,

    /// Tile number in VRAM tile data area.
    ///
    /// For 8x16 sprites, bit 0 is ignored (uses even tile for top, odd for bottom).
    pub tile: u8,

    /// Attribute flags (palette, priority, flip bits).
    ///
    /// Controls sprite rendering behavior including palette selection and flipping.
    pub flags: u8,
}

/// Display timing constants matching Game Boy hardware specifications.

/// Total scanlines per frame including VBlank period.
const LINES_PER_FRAME: u8 = 154;

/// CPU cycles per scanline (456 T-cycles).
const TICKS_PER_LINE: u32 = 456;

/// Visible scanlines (0-143).
const YRES: u8 = 144;

/// Pixels per scanline.
const XRES: u8 = 160;

/// Linked list node for sprite processing on current scanline.
///
/// Used to build sorted lists of sprites visible on the current scanline.
/// Sprites are sorted by X position for proper priority handling according
/// to Game Boy sprite priority rules.
///
/// # Priority Rules
///
/// When multiple sprites overlap:
/// 1. Sprite with smaller X coordinate has priority
/// 2. If X coordinates are equal, sprite with lower OAM index has priority
pub struct OAMLineEntry {
    /// Sprite attributes for this entry.
    pub entry: OAMEntry,

    /// Pointer to next sprite in sorted list.
    ///
    /// None indicates end of list.
    pub next: Option<Box<OAMLineEntry>>,
}

impl OAMLineEntry {
    /// Creates a new OAMLineEntry with the given sprite entry.
    ///
    /// # Arguments
    ///
    /// * `entry` - The OAM entry to wrap in this list node
    ///
    /// # Returns
    ///
    /// A new OAMLineEntry with next set to None
    pub fn new(entry: OAMEntry) -> Self {
        OAMLineEntry { entry, next: None }
    }
}

impl OAMEntry {
    /// Creates a new OAMEntry with all fields set to zero.
    ///
    /// # Returns
    ///
    /// A new OAMEntry with default values (sprite off-screen)
    pub fn new() -> Self {
        OAMEntry {
            y: 0,
            x: 0,
            tile: 0,
            flags: 0,
        }
    }

    /// Converts the OAM entry to a 4-byte array.
    ///
    /// # Returns
    ///
    /// Array of [Y, X, Tile, Flags]
    pub fn to_bytes(&self) -> [u8; 4] {
        [self.y, self.x, self.tile, self.flags]
    }

    /// Creates an OAM entry from a 4-byte array.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Array of [Y, X, Tile, Flags]
    ///
    /// # Returns
    ///
    /// A new OAMEntry with values from the byte array
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        OAMEntry {
            y: bytes[0],
            x: bytes[1],
            tile: bytes[2],
            flags: bytes[3],
        }
    }
}

/// Picture Processing Unit controller managing all graphics rendering.
///
/// Main PPU controller that manages all graphics rendering operations.
/// Coordinates between LCD timing, pixel pipeline, sprite processing,
/// and frame generation to produce authentic Game Boy video output.
///
/// # PPU Modes
///
/// The PPU operates in four distinct modes during each frame:
/// - Mode 2 (OAM Scan): Find sprites for current scanline (80 cycles)
/// - Mode 3 (Pixel Transfer): Render pixels using background, window, and sprites (172 cycles)
/// - Mode 0 (HBlank): Horizontal blanking between scanlines (204 cycles)
/// - Mode 1 (VBlank): Vertical blanking between frames (4560 cycles, 10 scanlines)
///
/// # Memory Layout
///
/// - VRAM: 8KB (0x8000-0x9FFF) for tile data and tile maps
/// - OAM: 160 bytes (0xFE00-0xFE9F) for 40 sprite entries
/// - Video Buffer: 160x144 pixels (23,040 32-bit ARGB values)
///
/// # Frame Timing
///
/// - Target: 60 FPS (16.67ms per frame)
/// - Total cycles per frame: 70,224 (154 scanlines × 456 cycles)
/// - Visible scanlines: 144 (0-143)
/// - VBlank scanlines: 10 (144-153)
pub struct PPU {
    /// Object Attribute Memory - 40 sprite entries.
    pub oam_ram: [OAMEntry; 40],

    /// Video RAM - 8KB for tile data and tile maps.
    pub vram: [u8; 0x2000],

    /// Current scanline being rendered (0-153).
    pub ly: u8,

    /// Current frame number for tracking.
    pub current_frame: u32,

    /// Video buffer for frame (160x144 32-bit ARGB pixels).
    pub video_buffer: Vec<u32>,

    /// Cycle counter for current scanline (0-455).
    pub line_ticks: u32,

    /// LCD controller managing display timing and modes.
    pub lcd: LCD,

    /// Pixel FIFO for background/window/sprite pixel processing.
    pub pixel_fifo: PixelFIFO,

    /// Number of sprites on current scanline (0-10 max).
    pub line_sprite_count: u8,

    /// Linked list of sprites visible on current scanline.
    pub line_sprites: Option<Box<OAMLineEntry>>,

    /// Array storage for line sprite entries.
    pub line_entry_array: [OAMLineEntry; 10],

    /// Number of sprite entries fetched for current tile.
    pub fetched_entry_count: u8,

    /// Sprite entries fetched for current tile (max 3).
    pub fetched_entries: [OAMEntry; 3],

    /// Window internal line counter for window rendering.
    pub window_line: u8,

    /// Target frame time in milliseconds (16.67ms for 60 FPS).
    target_frame_time: u32,

    /// Previous frame timestamp for frame pacing.
    prev_frame_time: u64,

    /// Start time for FPS calculation.
    start_timer: u64,

    /// Frame counter for FPS calculation.
    frame_count: u32,

    /// Current frames per second.
    pub current_fps: u32,
}

impl PPU {
    /// Creates a new PPU with default initialization.
    ///
    /// Initializes all PPU components including VRAM, OAM, LCD controller,
    /// pixel FIFO, and frame timing. Sets initial LCD mode to OAM scan.
    ///
    /// # Returns
    ///
    /// A new PPU instance ready for rendering
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

    /// Processes the pixel pipeline for the current cycle.
    ///
    /// Updates map coordinates, tile coordinates, and processes pipeline
    /// fetch and push operations. Handles window visibility and coordinate
    /// calculations for both background and window layers.
    ///
    /// Pipeline processing occurs on even cycles only for accurate timing.
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

    /// Fetches tile and pixel data for the pipeline.
    ///
    /// Implements the FIFO state machine for fetching background, window,
    /// and sprite data. Progresses through states: TILE → DATA0 → DATA1 → IDLE → PUSH.
    ///
    /// # Pipeline States
    ///
    /// - TILE: Fetch tile index from tile map
    /// - DATA0: Fetch first byte of tile data (low bit plane)
    /// - DATA1: Fetch second byte of tile data (high bit plane)
    /// - IDLE: Wait one cycle before pushing
    /// - PUSH: Push pixels to FIFO
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

    /// Reads a byte from VRAM.
    ///
    /// # Arguments
    ///
    /// * `address` - VRAM address (0x8000-0x9FFF)
    ///
    /// # Returns
    ///
    /// The byte at the specified VRAM address, or 0xFF if address is out of range
    fn read_vram(&self, address: u16) -> u8 {
        if address >= 0x8000 && address <= 0x9FFF {
            self.vram[(address - 0x8000) as usize]
        } else {
            0xFF
        }
    }

    /// Increments the LY register and handles LYC coincidence.
    ///
    /// Updates the window line counter if window is visible and being rendered.
    /// Checks for LY=LYC coincidence and triggers STAT interrupt if enabled.
    ///
    /// # Returns
    ///
    /// Vector of interrupts to request (LCDSTAT if LYC coincidence occurs)
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

    /// Handles PPU Mode 2 (OAM Scan).
    ///
    /// Scans OAM for sprites visible on the current scanline. Transitions to
    /// Transfer mode after 80 cycles. Loads sprite list on first tick.
    ///
    /// # Returns
    ///
    /// Empty vector (no interrupts generated in OAM mode)
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

    /// Handles PPU Mode 3 (Pixel Transfer).
    ///
    /// Processes the pixel pipeline to render background, window, and sprite pixels.
    /// Transitions to HBlank mode after rendering all 160 pixels.
    ///
    /// # Returns
    ///
    /// Vector of interrupts to request (LCDSTAT if HBlank interrupt enabled)
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

    /// Handles PPU Mode 1 (VBlank).
    ///
    /// Processes VBlank period (scanlines 144-153). Increments LY each scanline
    /// and transitions back to OAM mode when frame completes.
    ///
    /// # Returns
    ///
    /// Vector of interrupts to request (LCDSTAT if LYC coincidence occurs)
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

    /// Handles PPU Mode 0 (HBlank).
    ///
    /// Processes horizontal blanking period between scanlines. Increments LY
    /// after 456 cycles and transitions to either VBlank (after line 143) or
    /// OAM mode (for next scanline). Handles frame timing, FPS calculation,
    /// and battery saves.
    ///
    /// # Arguments
    ///
    /// * `cart` - Mutable reference to cartridge for battery saves
    ///
    /// # Returns
    ///
    /// Vector of interrupts to request (VBLANK, LCDSTAT)
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

    /// Advances the PPU by one T-cycle.
    ///
    /// Increments the line tick counter and dispatches to the appropriate
    /// mode handler based on current LCD mode. This is the main PPU entry
    /// point called once per CPU T-cycle.
    ///
    /// # Arguments
    ///
    /// * `cart` - Mutable reference to cartridge for battery saves during VBlank
    ///
    /// # Returns
    ///
    /// Vector of interrupts to request (VBLANK, LCDSTAT)
    pub fn ppu_tick(&mut self, cart: &mut crate::hdw::cart::Cartridge) -> Vec<Interrupts> {
        self.line_ticks += 1;

        match self.lcd.lcds_mode() {
            LcdMode::OAM => self.ppu_mode_oam(),
            LcdMode::Transfer => self.ppu_mode_xfer(),
            LcdMode::VBlank => self.ppu_mode_vblank(),
            LcdMode::HBlank => self.ppu_mode_hblank(cart),
        }
    }

    /// Updates the LCD LY register from PPU LY value.
    ///
    /// Synchronizes the LCD controller's LY register with the PPU's internal
    /// LY counter. Called after PPU processing to keep registers in sync.
    pub fn update_lcd_ly(&mut self) {
        self.lcd.ly = self.ly;
    }

    /// Writes a byte to OAM memory.
    ///
    /// # Arguments
    ///
    /// * `address` - OAM address (0xFE00-0xFE9F or 0x0000-0x009F)
    /// * `value` - Byte value to write
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

    /// Reads a byte from OAM memory.
    ///
    /// # Arguments
    ///
    /// * `address` - OAM address (0xFE00-0xFE9F or 0x0000-0x009F)
    ///
    /// # Returns
    ///
    /// The byte at the specified OAM address
    pub fn ppu_oam_read(&self, mut address: u16) -> u8 {
        if address >= 0xFE00 {
            address -= 0xFE00;
        }
        let entry_index = (address / 4) as usize;
        let byte_index = (address % 4) as usize;
        self.oam_ram[entry_index].to_bytes()[byte_index]
    }

    /// Writes a byte to VRAM.
    ///
    /// # Arguments
    ///
    /// * `address` - VRAM address (0x8000-0x9FFF)
    /// * `value` - Byte value to write
    pub fn ppu_vram_write(&mut self, address: u16, value: u8) {
        self.vram[(address - 0x8000) as usize] = value;
    }

    /// Reads a byte from VRAM.
    ///
    /// # Arguments
    ///
    /// * `address` - VRAM address (0x8000-0x9FFF)
    ///
    /// # Returns
    ///
    /// The byte at the specified VRAM address
    pub fn ppu_vram_read(&self, address: u16) -> u8 {
        self.vram[(address - 0x8000) as usize]
    }

    /// Loads sprites visible on the current scanline into sorted list.
    ///
    /// Scans all 40 OAM entries and builds a linked list of up to 10 sprites
    /// that are visible on the current scanline. Sprites are sorted by X position
    /// for proper priority handling.
    ///
    /// # Sprite Selection
    ///
    /// - Maximum 10 sprites per scanline
    /// - Sprites with X=0 are skipped (off-screen)
    /// - Y position checked against current scanline with 16-pixel offset
    /// - Sorted by X position (lower X = higher priority)
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

    /// Adds fetched pixels to the FIFO.
    ///
    /// Converts tile data to pixels and pushes them to the FIFO. Handles
    /// background/window pixel generation and sprite overlay. Applies scroll
    /// offset for proper pixel alignment.
    ///
    /// # Returns
    ///
    /// true if pixels were successfully added, false if FIFO is too full
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

    /// Pushes a pixel from the FIFO to the video buffer.
    ///
    /// Pops a pixel from the FIFO and writes it to the video buffer at the
    /// current screen position. Handles scroll offset and bounds checking.
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

    /// Loads sprite tiles that overlap with the current fetch position.
    ///
    /// Scans the line sprite list and identifies up to 3 sprites that overlap
    /// with the current 8-pixel fetch window. Stores these sprites for data
    /// fetching in subsequent pipeline stages.
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

    /// Loads sprite pixel data for fetched sprites.
    ///
    /// Fetches one byte of sprite tile data for each sprite in the fetched
    /// entries list. Handles Y-flip and 8x16 sprite mode.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset (0 for low bit plane, 1 for high bit plane)
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

    /// Fetches sprite pixel color for the current FIFO position.
    ///
    /// Checks all fetched sprites to find the highest priority visible sprite
    /// pixel at the current position. Handles sprite priority, transparency,
    /// X-flip, and palette selection.
    ///
    /// # Arguments
    ///
    /// * `_bit` - Bit position (unused, kept for compatibility)
    /// * `color` - Background/window color to potentially override
    /// * `bg_color` - Background color index for priority checking
    ///
    /// # Returns
    ///
    /// Final pixel color (sprite color if visible, otherwise background color)
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

    /// Loads window tile data into the pipeline.
    ///
    /// Fetches the tile index from the window tile map based on window-relative
    /// coordinates. Uses the window line counter for accurate window rendering.
    ///
    /// Only loads if window is visible and within bounds.
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

    /// Checks if the window layer is visible.
    ///
    /// # Returns
    ///
    /// true if window is enabled and WY is within visible range
    pub fn window_visible(&self) -> bool {
        self.lcd.lcdc_win_enable() && self.lcd.wy < YRES
    }

    /// Checks and updates window state.
    ///
    /// Resets the window line counter at the start of each frame (LY=0).
    /// This ensures proper window rendering across frames.
    pub fn check_window_state(&mut self) {
        // Reset window line counter at the start of each frame
        if self.ly == 0 {
            self.window_line = 0;
        }
    }
}
