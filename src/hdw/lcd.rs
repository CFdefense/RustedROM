// hdw/lcd.rs
// Game Boy LCD Controller Implementation
//
// This module implements the Game Boy's LCD (Liquid Crystal Display) controller,
// which manages the display timing, graphics modes, and visual output parameters.
// The LCD controller coordinates with the PPU to generate the final video signal.
//
// # Key Registers
//
// - LCDC (0xFF40): LCD Control - enables/disables display features
// - LCDS (0xFF41): LCD Status - current mode and interrupt sources
// - SCY/SCX (0xFF42/43): Background scroll registers
// - LY/LYC (0xFF44/45): Current scanline and scanline compare
// - WY/WX (0xFF4A/4B): Window position registers
// - BGP/OBP0/OBP1 (0xFF47-49): Palette data for colors
//
// # Display Modes
//
// - HBlank (Mode 0): Horizontal blanking - CPU can access VRAM/OAM
// - VBlank (Mode 1): Vertical blanking - CPU can access VRAM/OAM
// - OAM (Mode 2): OAM scan - CPU cannot access OAM
// - Transfer (Mode 3): Pixel transfer - CPU cannot access VRAM/OAM
//
// # Graphics Features
//
// - 160x144 pixel display with 4-shade grayscale
// - Background layer with infinite scrolling
// - Window overlay layer for UI elements
// - 40 hardware sprites with size/palette/priority control
// - Programmable palettes for authentic Game Boy colors
//
// # Interrupt Sources
//
// The LCD controller can generate STAT interrupts based on:
// - HBlank entry, VBlank entry, OAM mode entry
// - LY == LYC scanline coincidence detection
//
// The LCD system provides cycle-accurate timing and mode switching
// to ensure proper game compatibility and visual authenticity.

/// LCD controller display modes.
///
/// Represents the four distinct operating modes of the LCD controller.
/// Each mode has specific timing characteristics and memory access restrictions.
/// The PPU cycles through these modes during each frame to render graphics.
///
/// # Mode Timing (per scanline)
///
/// - Mode 2 (OAM): 80 cycles - Scanning sprite attributes
/// - Mode 3 (Transfer): 172 cycles - Transferring pixels to LCD
/// - Mode 0 (HBlank): 204 cycles - Horizontal blanking period
/// - Mode 1 (VBlank): 4560 cycles total (10 scanlines) - Vertical blanking period
///
/// Total frame time: 70224 cycles (approximately 59.7 Hz)
pub enum LcdMode {
    /// Mode 0: Horizontal blanking (204 cycles).
    ///
    /// During HBlank, the CPU can freely access VRAM and OAM. This is the safest
    /// time for graphics updates. STAT interrupt can be triggered on HBlank entry.
    HBlank = 0,

    /// Mode 1: Vertical blanking (4560 cycles total).
    ///
    /// During VBlank, the CPU can freely access VRAM and OAM. This period lasts
    /// for 10 scanlines (144-153). Both VBLANK and STAT interrupts can trigger.
    VBlank = 1,

    /// Mode 2: OAM scan (80 cycles).
    ///
    /// The PPU scans OAM to find sprites for the current scanline. CPU cannot
    /// access OAM during this mode. STAT interrupt can be triggered on OAM entry.
    OAM = 2,

    /// Mode 3: Pixel transfer (172 cycles).
    ///
    /// The PPU transfers pixels to the LCD. CPU cannot access VRAM or OAM during
    /// this mode. This is the most restrictive mode for memory access.
    Transfer = 3,
}

/// LCD Status interrupt sources.
///
/// Bit flags for different interrupt sources in the STAT register (0xFF41).
/// Multiple sources can be enabled simultaneously by setting their corresponding
/// bits. When enabled and the condition occurs, a STAT interrupt is triggered.
///
/// # STAT Register Format
///
/// - Bit 6: LYC=LY Coincidence Interrupt
/// - Bit 5: Mode 2 OAM Interrupt
/// - Bit 4: Mode 1 VBlank Interrupt
/// - Bit 3: Mode 0 HBlank Interrupt
/// - Bit 2: LYC=LY Flag (read-only)
/// - Bits 1-0: Mode Flag (read-only)
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)]
pub enum StatSrc {
    /// HBlank interrupt enable (bit 3).
    ///
    /// When set, triggers STAT interrupt on entry to HBlank mode (Mode 0).
    HBlank = 1 << 3,

    /// VBlank interrupt enable (bit 4).
    ///
    /// When set, triggers STAT interrupt on entry to VBlank mode (Mode 1).
    VBlank = 1 << 4,

    /// OAM interrupt enable (bit 5).
    ///
    /// When set, triggers STAT interrupt on entry to OAM scan mode (Mode 2).
    OAM = 1 << 5,

    /// LYC=LY coincidence interrupt enable (bit 6).
    ///
    /// When set, triggers STAT interrupt when LY matches LYC register value.
    LYC = 1 << 6,
}

/// LCD controller state and registers.
///
/// Manages all LCD control registers, display timing, and color palettes.
/// Provides hardware-accurate register access and palette management
/// for authentic Game Boy graphics output.
///
/// # Register Map
///
/// - 0xFF40 (LCDC): LCD Control - Display enable and feature flags
/// - 0xFF41 (LCDS): LCD Status - Mode and interrupt configuration
/// - 0xFF42 (SCY): Scroll Y - Background vertical scroll position
/// - 0xFF43 (SCX): Scroll X - Background horizontal scroll position
/// - 0xFF44 (LY): LCD Y Coordinate - Current scanline being drawn
/// - 0xFF45 (LYC): LY Compare - Scanline comparison for interrupts
/// - 0xFF46 (DMA): DMA Transfer - Initiates OAM DMA transfer
/// - 0xFF47 (BGP): BG Palette - Background color palette
/// - 0xFF48 (OBP0): Object Palette 0 - Sprite palette 0
/// - 0xFF49 (OBP1): Object Palette 1 - Sprite palette 1
/// - 0xFF4A (WY): Window Y - Window layer Y position
/// - 0xFF4B (WX): Window X - Window layer X position
///
/// # Color Palettes
///
/// The LCD maintains four color arrays for rendering:
/// - bg_colors: Background layer palette (4 shades)
/// - sp1_colors: Sprite palette 0 (4 shades, color 0 transparent)
/// - sp2_colors: Sprite palette 1 (4 shades, color 0 transparent)
/// - default_colors: Base grayscale palette (customizable)
pub struct LCD {
    /// LCD Control register (0xFF40).
    ///
    /// Controls display enable and various graphics features.
    pub lcdc: u8,

    /// LCD Status register (0xFF41).
    ///
    /// Contains current mode and interrupt enable flags.
    pub lcds: u8,

    /// Scroll Y register (0xFF42).
    ///
    /// Background layer vertical scroll position.
    pub scy: u8,

    /// Scroll X register (0xFF43).
    ///
    /// Background layer horizontal scroll position.
    pub scx: u8,

    /// LY register (0xFF44).
    ///
    /// Current scanline being drawn (0-153).
    pub ly: u8,

    /// LY Compare register (0xFF45).
    ///
    /// Scanline value for LYC=LY coincidence interrupt.
    pub lyc: u8,

    /// DMA Transfer register (0xFF46).
    ///
    /// Writing to this register initiates OAM DMA transfer.
    pub dma: u8,

    /// BG Palette Data register (0xFF47).
    ///
    /// Background color palette mapping (2 bits per color).
    pub bgp: u8,

    /// Object Palette 0 Data register (0xFF48).
    ///
    /// Sprite palette 0 mapping (color 0 is transparent).
    pub obp0: u8,

    /// Object Palette 1 Data register (0xFF49).
    ///
    /// Sprite palette 1 mapping (color 0 is transparent).
    pub obp1: u8,

    /// Window Y Position register (0xFF4A).
    ///
    /// Y coordinate for window layer display.
    pub wy: u8,

    /// Window X Position register (0xFF4B).
    ///
    /// X coordinate for window layer display (offset by 7).
    pub wx: u8,

    /// Background color palette (4 RGBA colors).
    ///
    /// Actual colors used for background rendering based on BGP register.
    pub bg_colors: [u32; 4],

    /// Sprite palette 0 colors (4 RGBA colors).
    ///
    /// Actual colors used for sprite rendering with OBP0 register.
    pub sp1_colors: [u32; 4],

    /// Sprite palette 1 colors (4 RGBA colors).
    ///
    /// Actual colors used for sprite rendering with OBP1 register.
    pub sp2_colors: [u32; 4],

    /// Default grayscale palette (4 RGBA colors).
    ///
    /// Base colors used for palette mapping. Can be customized for different
    /// Game Boy color schemes (original, pocket, light, etc.).
    pub default_colors: [u32; 4],
}

impl LCD {
    /// Creates a new LCD controller with Game Boy power-on defaults.
    ///
    /// Initializes all registers to their power-on state and sets up default
    /// grayscale color palettes. The default colors are white, light gray,
    /// dark gray, and black (0xFFFFFFFF, 0xFFAAAAAA, 0xFF555555, 0xFF000000).
    ///
    /// # Power-On Register Values
    ///
    /// - LCDC: 0x91 (LCD enabled, BG on, sprites 8x8)
    /// - LCDS: 0x85 (Mode 1, LYC flag set)
    /// - BGP: 0xFC (Background palette: 3,3,2,0)
    /// - OBP0/OBP1: 0xFF (Sprite palettes: 3,3,3,3)
    /// - All other registers: 0x00
    ///
    /// # Returns
    ///
    /// A new LCD controller with default initialization
    pub fn new() -> Self {
        let mut lcd: LCD = LCD {
            lcdc: 0x91, // Default value on startup
            lcds: 0x85, // Default value on startup
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            dma: 0,
            bgp: 0xFC, // Default palette
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            bg_colors: [0; 4],
            sp1_colors: [0; 4],
            sp2_colors: [0; 4],
            default_colors: [0xFFFFFFFF, 0xFFAAAAAA, 0xFF555555, 0xFF000000],
        };

        // assign default colors
        for i in 0..=3 {
            lcd.bg_colors[i] = lcd.default_colors[i];
            lcd.sp1_colors[i] = lcd.default_colors[i];
            lcd.sp2_colors[i] = lcd.default_colors[i];
        }

        lcd
    }

    /// Reads a value from an LCD register.
    ///
    /// Routes read requests to the appropriate LCD register based on address offset
    /// from 0xFF40. Returns 0xFF for invalid register addresses.
    ///
    /// # Arguments
    ///
    /// * `address` - LCD register address (0xFF40-0xFF4B)
    ///
    /// # Returns
    ///
    /// The current value of the specified LCD register
    pub fn lcd_read(&self, address: u16) -> u8 {
        let offset = (address - 0xFF40) as u8;

        match offset {
            0x00 => self.lcdc, // 0xFF40 - LCD Control
            0x01 => self.lcds, // 0xFF41 - LCD Status
            0x02 => self.scy,  // 0xFF42 - Scroll Y
            0x03 => self.scx,  // 0xFF43 - Scroll X
            0x04 => self.ly,   // 0xFF44 - LY
            0x05 => self.lyc,  // 0xFF45 - LY Compare
            0x06 => self.dma,  // 0xFF46 - DMA Transfer
            0x07 => self.bgp,  // 0xFF47 - BG Palette
            0x08 => self.obp0, // 0xFF48 - Object Palette 0
            0x09 => self.obp1, // 0xFF49 - Object Palette 1
            0x0A => self.wy,   // 0xFF4A - Window Y
            0x0B => self.wx,   // 0xFF4B - Window X
            _ => 0xFF,         // Invalid offset
        }
    }

    /// Writes a value to an LCD register.
    ///
    /// Routes write requests to the appropriate LCD register based on address offset
    /// from 0xFF40. Handles side effects like palette updates and DMA initiation.
    /// Writing to the DMA register (0xFF46) returns Some(value) to signal DMA start.
    ///
    /// # Arguments
    ///
    /// * `address` - LCD register address (0xFF40-0xFF4B)
    /// * `value` - Value to write to the register
    ///
    /// # Returns
    ///
    /// Some(value) if DMA should be initiated (0xFF46 write), None otherwise
    pub fn lcd_write(&mut self, address: u16, value: u8) -> Option<u8> {
        let offset = (address - 0xFF40) as u8;

        match offset {
            0x00 => {
                self.lcdc = value;
                None
            } // 0xFF40 - LCD Control
            0x01 => {
                self.lcds = value;
                None
            } // 0xFF41 - LCD Status
            0x02 => {
                self.scy = value;
                None
            } // 0xFF42 - Scroll Y
            0x03 => {
                self.scx = value;
                None
            } // 0xFF43 - Scroll X
            0x04 => {
                self.ly = value;
                None
            } // 0xFF44 - LY (typically read-only, but allowing write)
            0x05 => {
                self.lyc = value;
                None
            } // 0xFF45 - LY Compare
            0x06 => {
                self.dma = value;
                Some(value)
            } // 0xFF46 - DMA Transfer - return value to start DMA
            0x07 => {
                self.bgp = value;
                self.update_palette(value, 0);
                None
            } // 0xFF47 - BG Palette
            0x08 => {
                self.obp0 = value;
                self.update_palette(value & 0b11111100, 1);
                None
            } // 0xFF48 - Object Palette 0
            0x09 => {
                self.obp1 = value;
                self.update_palette(value & 0b11111100, 2);
                None
            } // 0xFF49 - Object Palette 1
            0x0A => {
                self.wy = value;
                None
            } // 0xFF4A - Window Y
            0x0B => {
                self.wx = value;
                None
            } // 0xFF4B - Window X
            _ => None, // Invalid offset - do nothing
        }
    }

    /// Updates palette colors based on palette register data.
    ///
    /// Converts 2-bit palette indices to actual RGBA colors using the default
    /// color palette. Each palette register contains four 2-bit indices that
    /// map to the four shades in default_colors.
    ///
    /// # Palette Data Format
    ///
    /// - Bits 0-1: Color 0 index
    /// - Bits 2-3: Color 1 index
    /// - Bits 4-5: Color 2 index
    /// - Bits 6-7: Color 3 index
    ///
    /// # Arguments
    ///
    /// * `palette_data` - 8-bit palette register value with four 2-bit indices
    /// * `pal` - Palette selector: 0=background, 1=sprite0, 2=sprite1
    fn update_palette(&mut self, palette_data: u8, pal: u8) {
        let p_colors = match pal {
            1 => &mut self.sp1_colors,
            2 => &mut self.sp2_colors,
            _ => &mut self.bg_colors, // Default case (0 and any other value)
        };

        p_colors[0] = self.default_colors[(palette_data & 0b11) as usize];
        p_colors[1] = self.default_colors[((palette_data >> 2) & 0b11) as usize];
        p_colors[2] = self.default_colors[((palette_data >> 4) & 0b11) as usize];
        p_colors[3] = self.default_colors[((palette_data >> 6) & 0b11) as usize];
    }

    // LCDC register bit checks

    /// Checks if background and window display is enabled (LCDC bit 0).
    ///
    /// When clear, background and window are disabled and only sprites are shown.
    /// On CGB, this bit controls priority instead of enable/disable.
    ///
    /// # Returns
    ///
    /// true if background/window is enabled, false otherwise
    pub fn lcdc_bgw_enable(&self) -> bool {
        self.bit(self.lcdc, 0)
    }

    /// Checks if sprite (object) display is enabled (LCDC bit 1).
    ///
    /// When clear, no sprites are displayed regardless of OAM contents.
    ///
    /// # Returns
    ///
    /// true if sprites are enabled, false otherwise
    pub fn lcdc_obj_enable(&self) -> bool {
        self.bit(self.lcdc, 1)
    }

    /// Returns the sprite height in pixels (LCDC bit 2).
    ///
    /// Sprites can be either 8x8 or 8x16 pixels. In 8x16 mode, two tiles
    /// are used per sprite (even tile number for top, odd for bottom).
    ///
    /// # Returns
    ///
    /// 16 if 8x16 sprite mode is enabled, 8 for 8x8 sprite mode
    pub fn lcdc_obj_height(&self) -> u8 {
        if self.bit(self.lcdc, 2) {
            16
        } else {
            8
        }
    }

    /// Returns the background tile map base address (LCDC bit 3).
    ///
    /// The background tile map is a 32x32 grid of tile indices that defines
    /// which tiles to display for the background layer.
    ///
    /// # Returns
    ///
    /// 0x9C00 if bit 3 is set, 0x9800 otherwise
    pub fn lcdc_bg_map_area(&self) -> u16 {
        if self.bit(self.lcdc, 3) {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Returns the background/window tile data base address (LCDC bit 4).
    ///
    /// Determines which addressing mode is used for tile data. In 0x8000 mode,
    /// tile indices are unsigned (0-255). In 0x8800 mode, indices are signed
    /// (-128 to 127) with base at 0x9000.
    ///
    /// # Returns
    ///
    /// 0x8000 if bit 4 is set (unsigned mode), 0x8800 otherwise (signed mode)
    pub fn lcdc_bgw_data_area(&self) -> u16 {
        if self.bit(self.lcdc, 4) {
            0x8000
        } else {
            0x8800
        }
    }

    /// Checks if window display is enabled (LCDC bit 5).
    ///
    /// The window is an overlay layer that can be positioned anywhere on screen.
    /// When enabled and visible, it overrides the background layer.
    ///
    /// # Returns
    ///
    /// true if window is enabled, false otherwise
    pub fn lcdc_win_enable(&self) -> bool {
        self.bit(self.lcdc, 5)
    }

    /// Returns the window tile map base address (LCDC bit 6).
    ///
    /// The window tile map is a 32x32 grid of tile indices that defines
    /// which tiles to display for the window layer.
    ///
    /// # Returns
    ///
    /// 0x9C00 if bit 6 is set, 0x9800 otherwise
    pub fn lcdc_win_map_area(&self) -> u16 {
        if self.bit(self.lcdc, 6) {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Checks if LCD display is enabled (LCDC bit 7).
    ///
    /// When clear, the LCD is off and the screen is blank. Turning off the LCD
    /// should only be done during VBlank to avoid hardware damage on real Game Boy.
    ///
    /// # Returns
    ///
    /// true if LCD is enabled, false otherwise
    #[allow(dead_code)]
    pub fn lcdc_lcd_enable(&self) -> bool {
        self.bit(self.lcdc, 7)
    }

    // LCDS register operations

    /// Returns the current LCD display mode (LCDS bits 0-1).
    ///
    /// The LCD cycles through four modes during each frame. Mode information
    /// is read-only and automatically updated by the PPU during rendering.
    ///
    /// # Returns
    ///
    /// The current LcdMode (HBlank, VBlank, OAM, or Transfer)
    pub fn lcds_mode(&self) -> LcdMode {
        match self.lcds & 0b11 {
            0 => LcdMode::HBlank,
            1 => LcdMode::VBlank,
            2 => LcdMode::OAM,
            3 => LcdMode::Transfer,
            _ => unreachable!(),
        }
    }

    /// Sets the LCD display mode (LCDS bits 0-1).
    ///
    /// Updates the mode bits in the LCDS register. This is typically called
    /// by the PPU during mode transitions. The mode bits are read-only from
    /// the CPU's perspective but writable internally.
    ///
    /// # Arguments
    ///
    /// * `mode` - The new LCD mode to set
    pub fn lcds_mode_set(&mut self, mode: LcdMode) {
        self.lcds &= !0b11;
        self.lcds |= mode as u8;
    }

    /// Returns the LYC=LY coincidence flag (LCDS bit 2).
    ///
    /// This flag is set when the LY register matches the LYC register value.
    /// Can be used to trigger STAT interrupts for scanline-specific effects.
    ///
    /// # Returns
    ///
    /// true if LY equals LYC, false otherwise
    #[allow(dead_code)]
    pub fn lcds_lyc(&self) -> bool {
        self.bit(self.lcds, 2)
    }

    /// Sets the LYC=LY coincidence flag (LCDS bit 2).
    ///
    /// Updates the coincidence flag based on whether LY matches LYC.
    /// This is typically called by the PPU when LY changes.
    ///
    /// # Arguments
    ///
    /// * `set` - true to set the flag, false to clear it
    pub fn lcds_lyc_set(&mut self, set: bool) {
        Self::bit_set(&mut self.lcds, 2, set);
    }

    /// Checks if a specific STAT interrupt source is enabled.
    ///
    /// Tests whether the specified interrupt source bit is set in the LCDS
    /// register. Multiple interrupt sources can be enabled simultaneously.
    ///
    /// # Arguments
    ///
    /// * `src` - The interrupt source to check (HBlank, VBlank, OAM, or LYC)
    ///
    /// # Returns
    ///
    /// true if the interrupt source is enabled, false otherwise
    pub fn lcds_stat_int(&self, src: StatSrc) -> bool {
        (self.lcds & src as u8) != 0
    }

    // Additional helper methods for interrupt management

    /// Checks if HBlank STAT interrupt is enabled.
    ///
    /// Convenience method for checking if STAT interrupt will trigger on
    /// entry to HBlank mode (Mode 0).
    ///
    /// # Returns
    ///
    /// true if HBlank interrupt is enabled, false otherwise
    #[allow(dead_code)]
    pub fn hblank_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::HBlank)
    }

    /// Checks if VBlank STAT interrupt is enabled.
    ///
    /// Convenience method for checking if STAT interrupt will trigger on
    /// entry to VBlank mode (Mode 1).
    ///
    /// # Returns
    ///
    /// true if VBlank interrupt is enabled, false otherwise
    #[allow(dead_code)]
    pub fn vblank_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::VBlank)
    }

    /// Checks if OAM STAT interrupt is enabled.
    ///
    /// Convenience method for checking if STAT interrupt will trigger on
    /// entry to OAM scan mode (Mode 2).
    ///
    /// # Returns
    ///
    /// true if OAM interrupt is enabled, false otherwise
    #[allow(dead_code)]
    pub fn oam_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::OAM)
    }

    /// Checks if LYC=LY coincidence STAT interrupt is enabled.
    ///
    /// Convenience method for checking if STAT interrupt will trigger when
    /// LY matches LYC register value.
    ///
    /// # Returns
    ///
    /// true if LYC interrupt is enabled, false otherwise
    #[allow(dead_code)]
    pub fn lyc_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::LYC)
    }

    /// Sets HBlank STAT interrupt enable flag.
    ///
    /// Enables or disables STAT interrupt triggering on HBlank entry.
    ///
    /// # Arguments
    ///
    /// * `enable` - true to enable interrupt, false to disable
    #[allow(dead_code)]
    pub fn set_hblank_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 3, enable);
    }

    /// Sets VBlank STAT interrupt enable flag.
    ///
    /// Enables or disables STAT interrupt triggering on VBlank entry.
    ///
    /// # Arguments
    ///
    /// * `enable` - true to enable interrupt, false to disable
    #[allow(dead_code)]
    pub fn set_vblank_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 4, enable);
    }

    /// Sets OAM STAT interrupt enable flag.
    ///
    /// Enables or disables STAT interrupt triggering on OAM scan entry.
    ///
    /// # Arguments
    ///
    /// * `enable` - true to enable interrupt, false to disable
    #[allow(dead_code)]
    pub fn set_oam_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 5, enable);
    }

    /// Sets LYC=LY coincidence STAT interrupt enable flag.
    ///
    /// Enables or disables STAT interrupt triggering on LY=LYC coincidence.
    ///
    /// # Arguments
    ///
    /// * `enable` - true to enable interrupt, false to disable
    #[allow(dead_code)]
    pub fn set_lyc_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 6, enable);
    }

    /// Updates the LYC=LY coincidence flag based on current register values.
    ///
    /// Compares the current LY and LYC register values and updates the
    /// coincidence flag accordingly. Should be called whenever LY changes.
    #[allow(dead_code)]
    pub fn update_lyc_flag(&mut self) {
        let lyc_equals_ly = self.ly == self.lyc;
        self.lcds_lyc_set(lyc_equals_ly);
    }

    /// Tests if a specific bit is set in a byte value.
    ///
    /// Helper function for checking individual bits in register values.
    ///
    /// # Arguments
    ///
    /// * `value` - The byte value to test
    /// * `bit` - The bit position to check (0-7)
    ///
    /// # Returns
    ///
    /// true if the bit is set, false otherwise
    fn bit(&self, value: u8, bit: u8) -> bool {
        (value & (1 << bit)) != 0
    }

    /// Sets or clears a specific bit in a byte value.
    ///
    /// Helper function for modifying individual bits in register values.
    ///
    /// # Arguments
    ///
    /// * `value` - Mutable reference to the byte value to modify
    /// * `bit` - The bit position to set/clear (0-7)
    /// * `set` - true to set the bit, false to clear it
    fn bit_set(value: &mut u8, bit: u8, set: bool) {
        if set {
            *value |= 1 << bit;
        } else {
            *value &= !(1 << bit);
        }
    }

    /// Updates the default color palette and refreshes all active palettes.
    ///
    /// Changes the base grayscale palette used for all color mapping and
    /// immediately updates the background and sprite palettes to reflect
    /// the new colors. Useful for switching between different Game Boy
    /// color schemes (original, pocket, light, etc.).
    ///
    /// # Arguments
    ///
    /// * `new_colors` - Array of 4 RGBA colors for the new default palette
    pub fn update_default_colors(&mut self, new_colors: [u32; 4]) {
        self.default_colors = new_colors;

        // Update all palettes with new default colors
        self.update_palette(self.bgp, 0);
        self.update_palette(self.obp0 & 0b11111100, 1);
        self.update_palette(self.obp1 & 0b11111100, 2);
    }
}
