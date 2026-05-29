/**
 * LCD Module - Game Boy LCD Controller Implementation
 *
 * This module implements the Game Boy's LCD (Liquid Crystal Display) controller,
 * which manages the display timing, graphics modes, and visual output parameters.
 * The LCD controller coordinates with the PPU to generate the final video signal.
 *
 * Key Registers:
 * - LCDC (0xFF40): LCD Control - enables/disables display features
 * - LCDS (0xFF41): LCD Status - current mode and interrupt sources  
 * - SCY/SCX (0xFF42/43): Background scroll registers
 * - LY/LYC (0xFF44/45): Current scanline and scanline compare
 * - WY/WX (0xFF4A/4B): Window position registers
 * - BGP/OBP0/OBP1 (0xFF47-49): Palette data for colors
 *
 * Display Modes:
 * - HBlank (Mode 0): Horizontal blanking - CPU can access VRAM/OAM
 * - VBlank (Mode 1): Vertical blanking - CPU can access VRAM/OAM
 * - OAM (Mode 2): OAM scan - CPU cannot access OAM  
 * - Transfer (Mode 3): Pixel transfer - CPU cannot access VRAM/OAM
 *
 * Graphics Features:
 * - 160x144 pixel display with 4-shade grayscale
 * - Background layer with infinite scrolling
 * - Window overlay layer for UI elements
 * - 40 hardware sprites with size/palette/priority control
 * - Programmable palettes for authentic Game Boy colors
 *
 * Interrupt Sources:
 * The LCD controller can generate STAT interrupts based on:
 * - HBlank entry, VBlank entry, OAM mode entry
 * - LY == LYC scanline coincidence detection
 *
 * The LCD system provides cycle-accurate timing and mode switching
 * to ensure proper game compatibility and visual authenticity.
 */

/**
 * LcdMode - LCD Controller Display Modes
 *
 * Represents the four distinct operating modes of the LCD controller.
 * Each mode has specific timing characteristics and memory access restrictions.
 */
pub enum LcdMode {
    /// Mode 0: Horizontal blanking (204 cycles)
    HBlank = 0,
    /// Mode 1: Vertical blanking (4560 cycles total)  
    VBlank = 1,
    /// Mode 2: OAM scan (80 cycles)
    OAM = 2,
    /// Mode 3: Pixel transfer (172 cycles)
    Transfer = 3,
}

/**
 * StatSrc - LCD Status Interrupt Sources
 *
 * Bit flags for different interrupt sources in the STAT register.
 * Multiple sources can be enabled simultaneously.
 */
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)]
pub enum StatSrc {
    /// HBlank interrupt enable (bit 3)
    HBlank = 1 << 3,
    /// VBlank interrupt enable (bit 4)
    VBlank = 1 << 4,
    /// OAM interrupt enable (bit 5)  
    OAM = 1 << 5,
    /// LYC=LY coincidence interrupt enable (bit 6)
    LYC = 1 << 6,
}

/**
 * LCD - LCD Controller State and Registers
 *
 * Manages all LCD control registers, display timing, and color palettes.
 * Provides hardware-accurate register access and palette management
 * for authentic Game Boy graphics output.
 */
pub struct LCD {
    pub lcdc: u8, // LCD Control register (0xFF40)
    pub lcds: u8, // LCD Status register (0xFF41)
    pub scy: u8,  // Scroll Y (0xFF42)
    pub scx: u8,  // Scroll X (0xFF43)
    pub ly: u8,   // LY - LCD Y coordinate (0xFF44)
    pub lyc: u8,  // LY Compare (0xFF45)
    pub dma: u8,  // DMA Transfer and Start Address (0xFF46)
    pub bgp: u8,  // BG Palette Data (0xFF47)
    pub obp0: u8, // Object Palette 0 Data (0xFF48)
    pub obp1: u8, // Object Palette 1 Data (0xFF49)
    pub wy: u8,   // Window Y Position (0xFF4A)
    pub wx: u8,   // Window X Position (0xFF4B)

    // Additional data
    pub bg_colors: [u32; 4],
    pub sp1_colors: [u32; 4],
    pub sp2_colors: [u32; 4],
    pub default_colors: [u32; 4],
}

impl LCD {
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

    /// Update palette colors based on palette data
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

    /// BG & Window enable/priority - LCDC_BGW_ENABLE
    pub fn lcdc_bgw_enable(&self) -> bool {
        self.bit(self.lcdc, 0)
    }

    /// Object enable - LCDC_OBJ_ENABLE  
    pub fn lcdc_obj_enable(&self) -> bool {
        self.bit(self.lcdc, 1)
    }

    /// Object height - LCDC_OBJ_HEIGHT
    pub fn lcdc_obj_height(&self) -> u8 {
        if self.bit(self.lcdc, 2) {
            16
        } else {
            8
        }
    }

    /// BG tile map area - LCDC_BG_MAP_AREA
    pub fn lcdc_bg_map_area(&self) -> u16 {
        if self.bit(self.lcdc, 3) {
            0x9C00
        } else {
            0x9800
        }
    }

    /// BG & Window tile data area - LCDC_BGW_DATA_AREA
    pub fn lcdc_bgw_data_area(&self) -> u16 {
        if self.bit(self.lcdc, 4) {
            0x8000
        } else {
            0x8800
        }
    }

    /// Window enable - LCDC_WIN_ENABLE
    pub fn lcdc_win_enable(&self) -> bool {
        self.bit(self.lcdc, 5)
    }

    /// Window tile map area - LCDC_WIN_MAP_AREA
    pub fn lcdc_win_map_area(&self) -> u16 {
        if self.bit(self.lcdc, 6) {
            0x9C00
        } else {
            0x9800
        }
    }

    /// LCD enable - LCDC_LCD_ENABLE
    #[allow(dead_code)]
    pub fn lcdc_lcd_enable(&self) -> bool {
        self.bit(self.lcdc, 7)
    }

    // LCDS register operations

    /// Get current LCD mode - LCDS_MODE
    pub fn lcds_mode(&self) -> LcdMode {
        match self.lcds & 0b11 {
            0 => LcdMode::HBlank,
            1 => LcdMode::VBlank,
            2 => LcdMode::OAM,
            3 => LcdMode::Transfer,
            _ => unreachable!(),
        }
    }

    /// Set LCD mode - LCDS_MODE_SET
    pub fn lcds_mode_set(&mut self, mode: LcdMode) {
        self.lcds &= !0b11;
        self.lcds |= mode as u8;
    }

    /// Get LYC flag - LCDS_LYC
    #[allow(dead_code)]
    pub fn lcds_lyc(&self) -> bool {
        self.bit(self.lcds, 2)
    }

    /// Set LYC flag - LCDS_LYC_SET
    pub fn lcds_lyc_set(&mut self, set: bool) {
        Self::bit_set(&mut self.lcds, 2, set);
    }

    /// Check if stat interrupt source is enabled - LCDS_STAT_INT
    pub fn lcds_stat_int(&self, src: StatSrc) -> bool {
        (self.lcds & src as u8) != 0
    }

    // Additional helper methods for interrupt management

    /// Check if HBlank interrupt is enabled
    #[allow(dead_code)]
    pub fn hblank_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::HBlank)
    }

    /// Check if VBlank interrupt is enabled
    #[allow(dead_code)]
    pub fn vblank_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::VBlank)
    }

    /// Check if OAM interrupt is enabled
    #[allow(dead_code)]
    pub fn oam_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::OAM)
    }

    /// Check if LYC interrupt is enabled
    #[allow(dead_code)]
    pub fn lyc_int_enabled(&self) -> bool {
        self.lcds_stat_int(StatSrc::LYC)
    }

    /// Set HBlank interrupt enable
    #[allow(dead_code)]
    pub fn set_hblank_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 3, enable);
    }

    /// Set VBlank interrupt enable
    #[allow(dead_code)]
    pub fn set_vblank_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 4, enable);
    }

    /// Set OAM interrupt enable
    #[allow(dead_code)]
    pub fn set_oam_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 5, enable);
    }

    /// Set LYC interrupt enable
    #[allow(dead_code)]
    pub fn set_lyc_int(&mut self, enable: bool) {
        Self::bit_set(&mut self.lcds, 6, enable);
    }

    /// Update LYC flag based on current LY and LYC values
    #[allow(dead_code)]
    pub fn update_lyc_flag(&mut self) {
        let lyc_equals_ly = self.ly == self.lyc;
        self.lcds_lyc_set(lyc_equals_ly);
    }

    // Helper function for bit checking
    fn bit(&self, value: u8, bit: u8) -> bool {
        (value & (1 << bit)) != 0
    }

    // Helper function for bit setting
    fn bit_set(value: &mut u8, bit: u8, set: bool) {
        if set {
            *value |= 1 << bit;
        } else {
            *value &= !(1 << bit);
        }
    }

    pub fn update_default_colors(&mut self, new_colors: [u32; 4]) {
        self.default_colors = new_colors;

        // Update all palettes with new default colors
        self.update_palette(self.bgp, 0);
        self.update_palette(self.obp0 & 0b11111100, 1);
        self.update_palette(self.obp1 & 0b11111100, 2);
    }
}
