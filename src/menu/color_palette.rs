#[derive(Debug, Clone, PartialEq)]
pub enum ColorPalette {
    ClassicGameBoy,
    GreenScale,
    PurpleShades,
    BlueShades,
    Sepia,
    RedShades,
    CyberpunkGreen,
    Ocean,
}

impl ColorPalette {
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
