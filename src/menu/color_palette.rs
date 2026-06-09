/*
  menu/color_palette.rs
  Info: Color palette system for Game Boy display emulation
  Description: Provides multiple color schemes to customize the Game Boy's 4-shade grayscale
              display. Each palette defines 4 colors in ARGB format, from lightest to darkest,
              allowing users to personalize their gaming experience with different visual themes.

  Color Format:
    - ARGB 32-bit format: 0xAARRGGBB
    - Alpha channel (AA): Always 0xFF (fully opaque)
    - RGB channels: Standard 8-bit color values

  Palette Organization:
    - Index 0: Lightest shade (white/light color)
    - Index 1: Light-medium shade
    - Index 2: Dark-medium shade
    - Index 3: Darkest shade (black/dark color)

  Available Palettes:
    - ClassicGameBoy: Authentic grayscale matching original hardware
    - GreenScale: Classic Game Boy green tint
    - PurpleShades: Purple/lavender theme
    - BlueShades: Ocean blue theme
    - Sepia: Vintage sepia tone
    - RedShades: Ruby red theme
    - CyberpunkGreen: Neon green cyberpunk aesthetic
    - Ocean: Deep ocean blue-green theme
*/

/// Color palette options for customizing the Game Boy display.
///
/// Each palette provides 4 colors representing the Game Boy's grayscale shades,
/// allowing users to personalize the visual appearance of games.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorPalette {
    /// Authentic grayscale matching original Game Boy hardware
    ClassicGameBoy,
    /// Classic Game Boy green tint (DMG-style)
    GreenScale,
    /// Purple and lavender color scheme
    PurpleShades,
    /// Ocean blue gradient theme
    BlueShades,
    /// Vintage sepia tone for retro aesthetic
    Sepia,
    /// Ruby red gradient theme
    RedShades,
    /// Neon green cyberpunk aesthetic
    CyberpunkGreen,
    /// Deep ocean blue-green theme
    Ocean,
}

impl ColorPalette {
    /// Returns the 4-color array for this palette in ARGB format.
    ///
    /// # Returns
    ///
    /// Array of 4 colors ordered from lightest (index 0) to darkest (index 3).
    /// Each color is a 32-bit ARGB value (0xAARRGGBB).
    ///
    /// # Example
    ///
    /// ```
    /// let palette = ColorPalette::ClassicGameBoy;
    /// let colors = palette.get_colors();
    ///  --> colors[0] = 0xFFFFFFFF (white)
    ///  --> colors[3] = 0xFF000000 (black)
    /// ```
    pub fn get_colors(&self) -> [u32; 4] {
        match self {
            ColorPalette::ClassicGameBoy => [
                0xFFFFFFFF, // White
                0xFFAAAAAA, // Light gray
                0xFF555555, // Dark gray
                0xFF000000, // Black
            ],
            ColorPalette::GreenScale => [
                0xFF9BBB0F, // Light green
                0xFF8BAC0F, // Medium green
                0xFF306230, // Dark green
                0xFF0F380F, // Very dark green
            ],
            ColorPalette::PurpleShades => [
                0xFFE6E6FA, // Lavender
                0xFFDDA0DD, // Plum
                0xFF9370DB, // Medium slate blue
                0xFF4B0082, // Indigo
            ],
            ColorPalette::BlueShades => [
                0xFFE0F6FF, // Alice blue
                0xFF87CEEB, // Sky blue
                0xFF4682B4, // Steel blue
                0xFF191970, // Midnight blue
            ],
            ColorPalette::Sepia => [
                0xFFFFF8DC, // Cornsilk
                0xFFDEB887, // Burlywood
                0xFFCD853F, // Peru
                0xFF8B4513, // Saddle brown
            ],
            ColorPalette::RedShades => [
                0xFFFFE4E1, // Misty rose
                0xFFFF6B6B, // Light red
                0xFFDC143C, // Crimson
                0xFF8B0000, // Dark red
            ],
            ColorPalette::CyberpunkGreen => [
                0xFF00FF41, // Bright neon green
                0xFF00CC33, // Medium neon green
                0xFF008F11, // Dark neon green
                0xFF003300, // Very dark green
            ],
            ColorPalette::Ocean => [
                0xFFF0F8FF, // Alice blue
                0xFF00CED1, // Dark turquoise
                0xFF008B8B, // Dark cyan
                0xFF2F4F4F, // Dark slate gray
            ],
        }
    }

    /// Returns the display name for this palette.
    ///
    /// # Returns
    ///
    /// A human-readable string name for the palette, suitable for display in menus.
    pub fn get_name(&self) -> &'static str {
        match self {
            ColorPalette::ClassicGameBoy => "Classic Game Boy",
            ColorPalette::GreenScale => "Green Scale",
            ColorPalette::PurpleShades => "Purple Dreams",
            ColorPalette::BlueShades => "Ocean Blue",
            ColorPalette::Sepia => "Vintage Sepia",
            ColorPalette::RedShades => "Ruby Red",
            ColorPalette::CyberpunkGreen => "Cyberpunk",
            ColorPalette::Ocean => "Deep Ocean",
        }
    }

    /// Returns a vector containing all available color palettes.
    ///
    /// # Returns
    ///
    /// Vector of all `ColorPalette` variants in display order.
    /// Useful for populating palette selection menus.
    pub fn all_palettes() -> Vec<ColorPalette> {
        vec![
            ColorPalette::ClassicGameBoy,
            ColorPalette::GreenScale,
            ColorPalette::PurpleShades,
            ColorPalette::BlueShades,
            ColorPalette::Sepia,
            ColorPalette::RedShades,
            ColorPalette::CyberpunkGreen,
            ColorPalette::Ocean,
        ]
    }
}
