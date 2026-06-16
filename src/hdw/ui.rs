// hdw/ui.rs
// SDL2-based Game Boy Emulator User Interface
//
// This module implements the complete user interface system for the Game Boy emulator,
// including the main game display, debug tile viewer, header overlay, and audio output.
// It uses SDL2 for cross-platform graphics, audio, and input handling.
//
// # Display Components
//
// - Main game window: 800x700 pixels with scaled Game Boy display (160x144 -> 640x576)
// - Debug tile viewer: 16x24 grid showing all 384 VRAM tiles (when debug enabled)
// - Header overlay: Shows game name, current time, and exit button
// - Footer bar: Shows FPS counter and control mappings
//
// # Audio System
//
// - SDL2 audio queue with 44.1kHz mono output
// - Real-time audio sample buffering from APU
// - Configurable buffer sizes for low-latency playback
// - Automatic silence filling when no samples available
//
// # Rendering Pipeline
//
// - Surface-based pixel manipulation for game display
// - Texture streaming for GPU-accelerated rendering
// - Custom 5x7 bitmap font rendering for UI text
// - Color palette support for authentic Game Boy visuals
//
// # Input Handling
//
// - Keyboard mapping: Z=B, X=A, Arrows=D-Pad, Enter=Start, Tab=Select, Esc=Exit
// - Mouse support for EXIT button in header
// - Event-driven input processing through SDL2
//
// The UI system provides both emulation display and development tools,
// with the debug viewer showing raw VRAM tile data for development purposes.

use crate::hdw::cpu::CPU;
use chrono::Local;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::surface::Surface;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::WindowContext;
use sdl2::EventPump;
use sdl2::VideoSubsystem;
use std::sync::MutexGuard;
use std::time::{SystemTime, UNIX_EPOCH};

/// Main emulator window width in pixels.
///
/// Provides space for the scaled Game Boy display plus UI elements.
pub const SCREEN_WIDTH: u32 = 800;

/// Main emulator window height in pixels.
///
/// Increased to 700 to ensure full visibility of game display with header and footer.
pub const SCREEN_HEIGHT: u32 = 700;

/// Game Boy LCD horizontal resolution in pixels.
///
/// The original Game Boy screen width.
pub const XRES: u32 = 160;

/// Game Boy LCD vertical resolution in pixels.
///
/// The original Game Boy screen height.
pub const YRES: u32 = 144;

/// Pixel upscaling factor for Game Boy display.
///
/// Each Game Boy pixel is rendered as a 4x4 block of screen pixels,
/// resulting in a 640x576 display area (160*4 x 144*4).
const SCALE: u32 = 4;

/// Debug window width in pixels.
///
/// Shows VRAM tile data in a 16x24 grid (384 tiles total).
/// Each 8x8 tile is scaled by SCALE factor: 16 * 8 * 4 = 512 pixels.
pub const DEBUG_WINDOW_WIDTH: u32 = 16 * 8 * SCALE;

/// Debug window height in pixels.
///
/// Shows VRAM tile data in a 16x24 grid (384 tiles total).
/// Each 8x8 tile is scaled by SCALE factor: 24 * 8 * 4 = 768 pixels.
pub const DEBUG_WINDOW_HEIGHT: u32 = 24 * 8 * SCALE;

/// Debug surface width matching window size.
///
/// Prevents black space by matching the debug window dimensions exactly.
pub const DEBUG_SURFACE_WIDTH: u32 = 16 * 8 * SCALE;

/// Debug surface height matching window size.
///
/// Prevents black space by matching the debug window dimensions exactly.
pub const DEBUG_SURFACE_HEIGHT: u32 = 24 * 8 * SCALE;

/// Color palette for tile display in debug viewer.
///
/// Represents the 4 possible Game Boy grayscale shades in ARGB format:
/// - 0xFFFFFFFF: White (color 0)
/// - 0xFFAAAAAA: Light gray (color 1)
/// - 0xFF555555: Dark gray (color 2)
/// - 0xFF000000: Black (color 3)
const TILE_COLORS: [u32; 4] = [0xFFFFFFFF, 0xFFAAAAAA, 0xFF555555, 0xFF000000];

/// Main user interface controller for the Game Boy emulator.
///
/// Manages all aspects of the emulator's visual and audio presentation.
/// Coordinates between SDL2 subsystems and emulator components to provide
/// real-time display of Game Boy output with optional debug visualizations.
///
/// # Display System
///
/// - Main window: 800x700 pixels with centered 640x576 game display
/// - Debug window: Optional 512x768 tile viewer showing all VRAM tiles
/// - Header bar: Game name, current time, and EXIT button
/// - Footer bar: FPS counter and control mappings
///
/// # Audio System
///
/// - 44.1kHz mono audio output through SDL2 audio queue
/// - Real-time sample buffering from APU with automatic silence filling
/// - Target buffer size of 4096 samples for smooth playback
///
/// # Rendering Pipeline
///
/// 1. PPU generates pixels to video buffer
/// 2. UI copies pixels to SDL surface with scaling
/// 3. Surface converted to texture for GPU rendering
/// 4. Texture presented to screen via canvas
///
/// The UI handles window management, rendering pipelines, audio streaming,
/// and provides development tools for debugging graphics and timing.
pub struct UI {
    /// SDL2 context (kept alive for subsystem lifetime).
    pub _sdl_context: sdl2::Sdl,

    /// SDL2 video subsystem (kept alive for window lifetime).
    pub _video_subsystem: VideoSubsystem,

    /// SDL2 TTF context for text rendering (currently unused).
    pub _ttf_context: Sdl2TtfContext,

    /// Main game window canvas for rendering.
    pub main_canvas: WindowCanvas,

    /// Optional debug tile viewer canvas (only when debug enabled).
    pub debug_canvas: Option<WindowCanvas>,

    /// Texture creator for main window rendering.
    pub main_texture_creator: TextureCreator<WindowContext>,

    /// Optional texture creator for debug window (only when debug enabled).
    pub debug_texture_creator: Option<TextureCreator<WindowContext>>,

    /// SDL2 event pump for input handling.
    pub event_pump: EventPump,

    /// Main display surface for pixel manipulation.
    ///
    /// Holds the complete frame including game display, header, and footer.
    pub screen_surface: Surface<'static>,

    /// Optional debug surface for tile viewer (only when debug enabled).
    pub debug_surface: Option<Surface<'static>>,

    /// SDL2 audio queue for sound output.
    ///
    /// None if audio initialization failed.
    pub audio_queue: Option<AudioQueue<f32>>,

    /// Debug mode flag.
    ///
    /// When true, enables debug tile viewer window.
    pub debug: bool,

    /// Current game name for header display.
    pub current_game_name: Option<String>,

    /// Header bar visibility flag.
    pub show_header: bool,

    /// Exit request flag set by user input.
    pub exit_requested: bool,

    /// Frame counter for FPS calculation.
    pub fps_counter: u32,

    /// Current FPS value for display.
    pub fps_display: u32,

    /// Timer for FPS calculation (milliseconds).
    pub fps_timer: u64,
}

impl UI {
    /// Creates a new UI instance with SDL2 initialization.
    ///
    /// Initializes all SDL2 subsystems (video, audio, TTF), creates the main game window,
    /// and optionally creates a debug tile viewer window. Sets up audio queue for sound
    /// output and prepares rendering surfaces.
    ///
    /// # Window Configuration
    ///
    /// - Main window: 800x700 pixels, centered on screen
    /// - Debug window: 512x768 pixels, positioned to the right of main window (if debug=true)
    /// - Both windows use ARGB8888 pixel format for full color support
    ///
    /// # Audio Configuration
    ///
    /// - Sample rate: 44.1kHz
    /// - Channels: Mono (1 channel)
    /// - Buffer size: 4096 samples
    ///
    /// # Arguments
    ///
    /// * `debug` - If true, creates debug tile viewer window
    ///
    /// # Returns
    ///
    /// Result containing the initialized UI or an error string if initialization fails
    ///
    /// # Errors
    ///
    /// Returns Err if SDL2 initialization fails, window creation fails, or surface
    /// allocation fails. Audio failure is non-fatal and results in None audio_queue.
    pub fn new(debug: bool) -> Result<Self, String> {
        // Initialize SDL2 video subsystem
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;
        let event_pump = sdl_context.event_pump()?;

        println!("SDL INIT");

        // Initialize SDL2 TTF for text rendering (though not currently used)
        let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;
        println!("TTF INIT");

        // Initialize SDL2 audio
        let audio_subsystem = sdl_context.audio()?;
        println!("AUDIO INIT");

        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1), // Mono
            samples: Some(4096),
        };

        let audio_queue = match audio_subsystem.open_queue::<f32, _>(None, &desired_spec) {
            Ok(queue) => {
                queue.resume(); // Start audio playback
                Some(queue)
            }
            Err(e) => {
                println!("Failed to initialize audio: {}", e);
                None
            }
        };

        // Create main emulator window - centered on screen
        let main_window = video_subsystem
            .window("GameBoy", SCREEN_WIDTH, SCREEN_HEIGHT)
            .position_centered()
            .build()
            .map_err(|e| e.to_string())?;

        // Create debug tile viewer window only if debug is enabled
        let (debug_canvas, debug_texture_creator, debug_surface) = if debug {
            // Get main window position to place debug window adjacent to it
            let (x, y) = main_window.position();

            // Create debug tile viewer window - positioned to the right of main window
            let debug_window = video_subsystem
                .window("Debug Viewer", DEBUG_WINDOW_WIDTH, DEBUG_WINDOW_HEIGHT)
                .position(x + SCREEN_WIDTH as i32 + 10, y)
                .build()
                .map_err(|e| e.to_string())?;

            let canvas = debug_window
                .into_canvas()
                .build()
                .map_err(|e| e.to_string())?;
            let texture_creator = canvas.texture_creator();

            // Create surface for debug tile display with extra space for padding
            let surface = Surface::new(
                DEBUG_SURFACE_WIDTH,
                DEBUG_SURFACE_HEIGHT,
                PixelFormatEnum::ARGB8888,
            )
            .map_err(|e| e.to_string())?;

            (Some(canvas), Some(texture_creator), Some(surface))
        } else {
            (None, None, None)
        };

        // Convert main window to canvas object for 2D rendering
        let main_canvas = main_window
            .into_canvas()
            .build()
            .map_err(|e| e.to_string())?;

        // Create texture creator for efficient GPU-accelerated rendering
        let main_texture_creator = main_canvas.texture_creator();

        // Create RGB surface for main display - ARGB8888 format for full color support
        let screen_surface = Surface::new(SCREEN_WIDTH, SCREEN_HEIGHT, PixelFormatEnum::ARGB8888)
            .map_err(|e| e.to_string())?;

        Ok(UI {
            _sdl_context: sdl_context,
            _video_subsystem: video_subsystem,
            _ttf_context: ttf_context,
            main_canvas,
            debug_canvas,
            main_texture_creator,
            debug_texture_creator,
            event_pump,
            screen_surface,
            debug_surface,
            audio_queue,
            debug,
            current_game_name: None,
            show_header: true,
            exit_requested: false,
            fps_counter: 0,
            fps_display: 0,
            fps_timer: 0,
        })
    }

    /// Renders a single 8x8 tile from VRAM to the debug surface.
    ///
    /// Each tile consists of 16 bytes (2 bytes per 8-pixel row). The two bytes form
    /// bit planes that combine to create 2-bit color values (0-3). The tile is rendered
    /// with pixel scaling applied.
    ///
    /// # Tile Data Format
    ///
    /// Each row of the tile uses 2 bytes:
    /// - Byte 1: Low bit plane (bits 0-7 for pixels 7-0)
    /// - Byte 2: High bit plane (bits 0-7 for pixels 7-0)
    /// - Combined: 2-bit color index per pixel (0=white, 3=black)
    ///
    /// # Arguments
    ///
    /// * `start_location` - Base address in VRAM (typically 0x8000)
    /// * `tile_num` - Tile index (0-383)
    /// * `x` - X position on debug surface
    /// * `y` - Y position on debug surface
    /// * `cpu` - CPU reference for VRAM access
    fn display_tile(
        &mut self,
        start_location: u16,
        tile_num: u16,
        x: i32,
        y: i32,
        cpu: &mut super::cpu::CPU,
    ) {
        // Only render if debug surface exists
        let debug_surface = if let Some(ref mut surface) = self.debug_surface {
            surface
        } else {
            return;
        };

        // Process each row of the tile (8 rows total, 2 bytes per row)
        for tile_y in (0..16).step_by(2) {
            // Calculate addresses for the two bit planes of this row
            let addr1 = start_location + (tile_num * 16) + tile_y as u16;
            let addr2 = start_location + (tile_num * 16) + tile_y as u16 + 1;

            // Ensure we're reading from valid VRAM range to prevent crashes
            if addr1 >= 0x8000 && addr1 <= 0x9FFF && addr2 >= 0x8000 && addr2 <= 0x9FFF {
                // Read the two bit planes for this row
                let b1 = cpu.bus.read_byte(None, addr1);
                let b2 = cpu.bus.read_byte(None, addr2);

                // Process each pixel in the row (8 pixels, from bit 7 down to bit 0)
                for bit in (0..=7).rev() {
                    // Extract bit from each plane and combine to form 2-bit color index
                    let hi = ((b1 & (1 << bit)) != 0) as u8 * 2; // High bit contributes 2 to value
                    let lo = ((b2 & (1 << bit)) != 0) as u8; // Low bit contributes 1 to value
                    let color = hi | lo; // Combine to get color index (0-3)

                    // Calculate pixel position on screen with scaling
                    let rect = Rect::new(
                        x + ((7 - bit) * SCALE as i32),    // X position (left to right)
                        y + ((tile_y / 2) * SCALE as i32), // Y position (top to bottom)
                        SCALE,                             // Width of scaled pixel
                        SCALE,                             // Height of scaled pixel
                    );

                    // Fill the scaled pixel rectangle with the appropriate color
                    if (color as usize) < TILE_COLORS.len() {
                        let color_value = TILE_COLORS[color as usize];
                        debug_surface
                            .fill_rect(
                                rect,
                                Color::RGBA(
                                    ((color_value >> 16) & 0xFF) as u8, // Red component
                                    ((color_value >> 8) & 0xFF) as u8,  // Green component
                                    (color_value & 0xFF) as u8,         // Blue component
                                    ((color_value >> 24) & 0xFF) as u8, // Alpha component
                                ),
                            )
                            .unwrap();
                    }
                }
            }
        }
    }

    /// Updates the debug window showing all tiles in VRAM.
    ///
    /// Displays all 384 tiles from VRAM in a 16x24 grid layout. Each tile is 8x8 pixels
    /// and is scaled by the SCALE factor. The debug surface is cleared with a dark gray
    /// background before rendering tiles.
    ///
    /// Only updates if debug mode is enabled and debug components exist.
    ///
    /// # Arguments
    ///
    /// * `cpu` - Mutable CPU reference for VRAM access
    pub fn update_dbg_window(&mut self, cpu: &mut super::cpu::CPU) {
        // Only update if debug is enabled and components exist
        if !self.debug
            || self.debug_surface.is_none()
            || self.debug_texture_creator.is_none()
            || self.debug_canvas.is_none()
        {
            return;
        }

        let mut x_draw = 0;
        let mut y_draw = 0;
        let mut tile_num = 0;

        // Clear debug surface with dark gray background
        if let Some(ref mut debug_surface) = self.debug_surface {
            debug_surface
                .fill_rect(None, Color::RGBA(0x11, 0x11, 0x11, 0xFF))
                .unwrap();
        }

        // Start from VRAM tile data area
        let addr = 0x8000;

        // Render all 384 tiles in a 16x24 grid
        for y in 0..24 {
            for x in 0..16 {
                // Render individual tile at calculated position
                self.display_tile(
                    addr,
                    tile_num,
                    x_draw + (x * SCALE as i32),
                    y_draw + (y * SCALE as i32),
                    cpu,
                );
                // Move to next horizontal tile position
                x_draw += (8 * SCALE) as i32;
                // Move to next tile number
                tile_num += 1;
            }
            // Move to next row of tiles
            y_draw += (8 * SCALE) as i32;
            // Reset horizontal position for new row
            x_draw = 0;
        }

        // Create texture from surface and render to debug window
        if let (
            Some(ref debug_texture_creator),
            Some(ref mut debug_canvas),
            Some(ref debug_surface),
        ) = (
            &self.debug_texture_creator,
            &mut self.debug_canvas,
            &self.debug_surface,
        ) {
            let debug_texture = debug_texture_creator
                .create_texture_from_surface(debug_surface)
                .expect("Failed to create debug texture");

            debug_canvas.clear();
            debug_canvas.copy(&debug_texture, None, None).unwrap();
            debug_canvas.present();
        }
    }

    /// Updates the main game display window.
    ///
    /// Renders the PPU's video buffer to screen with pixel scaling, draws header and
    /// footer overlays, and updates the debug window if enabled. The game display is
    /// centered in the window with appropriate padding for UI elements.
    ///
    /// # Rendering Steps
    ///
    /// 1. Update debug window (if enabled)
    /// 2. Update FPS counter
    /// 3. Clear screen surface
    /// 4. Calculate centering offsets
    /// 5. Render each pixel from video buffer with scaling
    /// 6. Render header bar (if enabled)
    /// 7. Render footer bar with FPS and controls
    /// 8. Convert surface to texture and present
    ///
    /// # Arguments
    ///
    /// * `cpu` - Mutable CPU reference for PPU video buffer access
    pub fn ui_update(&mut self, cpu: &mut super::cpu::CPU) {
        // Update debug window first to avoid borrow conflicts
        self.update_dbg_window(cpu);

        // Update FPS counter
        self.update_fps();

        // Clear the screen with black background
        self.screen_surface
            .fill_rect(None, Color::RGB(0, 0, 0))
            .unwrap();

        // Define padding constants - minimal padding for maximum game size
        let header_height = 35u32; // Keep header height the same
        let bottom_padding = 60u32; // Increased from 30 to 60 for more space at bottom

        // Calculate centering offsets to center the game in the window
        let game_width = XRES * SCALE;
        let game_height = YRES * SCALE;
        let offset_x = (SCREEN_WIDTH - game_width) / 2;

        // Calculate vertical position with safe arithmetic
        let total_padding = header_height + bottom_padding;
        let remaining_height = SCREEN_HEIGHT - total_padding;
        let offset_y = if game_height <= remaining_height {
            header_height + (remaining_height - game_height) / 2
        } else {
            header_height // If game is too tall, just position at header height
        };

        // Draw dark gray background for game area
        let game_area_rect = Rect::new(offset_x as i32, offset_y as i32, game_width, game_height);
        self.screen_surface
            .fill_rect(game_area_rect, Color::RGB(20, 20, 20))
            .unwrap();

        // Render each pixel from the Game Boy's video buffer to the main display
        for line_num in 0..YRES {
            for x in 0..XRES {
                let buffer_index = (x + (line_num * XRES)) as usize;
                if buffer_index < cpu.bus.ppu.video_buffer.len() {
                    let pixel_color = cpu.bus.ppu.video_buffer[buffer_index];

                    // Calculate scaled pixel rectangle with centering offset
                    let rect = Rect::new(
                        (offset_x + x * SCALE) as i32,
                        (offset_y + line_num * SCALE) as i32,
                        SCALE,
                        SCALE,
                    );

                    // Draw scaled pixel with the color from video buffer
                    self.screen_surface
                        .fill_rect(
                            rect,
                            Color::RGBA(
                                ((pixel_color >> 16) & 0xFF) as u8,
                                ((pixel_color >> 8) & 0xFF) as u8,
                                (pixel_color & 0xFF) as u8,
                                0xFF, // Force full alpha for visibility
                            ),
                        )
                        .unwrap();
                }
            }
        }

        // Render header bar overlay if enabled
        if self.show_header {
            self.render_header_bar();
        }

        // Render footer bar with FPS and controls
        self.render_footer_bar();

        // Create texture from surface and render to main window
        let main_texture = self
            .main_texture_creator
            .create_texture_from_surface(&self.screen_surface)
            .expect("Failed to create main texture");

        self.main_canvas.clear();
        self.main_canvas.copy(&main_texture, None, None).unwrap();
        self.main_canvas.present();
    }

    /// Updates the FPS counter.
    ///
    /// Increments the frame counter and updates the display FPS value once per second.
    /// Uses millisecond timing to calculate frames per second.
    fn update_fps(&mut self) {
        let now = get_ticks();
        if now - self.fps_timer > 1000 {
            self.fps_display = self.fps_counter;
            self.fps_counter = 0;
            self.fps_timer = now;
        } else {
            self.fps_counter += 1;
        }
    }

    /// Sets the current game name for display in the header bar.
    ///
    /// # Arguments
    ///
    /// * `game_name` - Name of the currently loaded game
    pub fn set_game_name(&mut self, game_name: String) {
        self.current_game_name = Some(game_name);
    }

    /// Renders the header bar overlay with game name, time, and exit button.
    ///
    /// The header bar is a semi-transparent dark overlay at the top of the screen
    /// containing:
    /// - Game name on the left
    /// - Current time (HH:MM:SS) in the center
    /// - EXIT button on the right (clickable)
    ///
    /// The EXIT button has a red background with white text and border.
    fn render_header_bar(&mut self) {
        let header_height = 35; // Match the header height constant
        let header_rect = Rect::new(0, 0, SCREEN_WIDTH, header_height);

        // Draw semi-transparent dark background
        self.screen_surface
            .fill_rect(header_rect, Color::RGBA(0, 0, 0, 180))
            .unwrap();

        // Draw game name on the left with adjusted y position
        if let Some(ref game_name) = self.current_game_name {
            let game_name_clone = game_name.clone();
            self.draw_header_text(&game_name_clone, 20, 12, Color::RGB(255, 255, 255));
            // Adjusted y from 15 to 12
        }

        // Draw current time in the center with adjusted y position
        let time_str = self.get_current_time_string();
        let time_width = time_str.len() as i32 * 6; // 6 pixels per character
        let center_x = (SCREEN_WIDTH as i32 / 2) - (time_width / 2);
        self.draw_header_text(&time_str, center_x, 12, Color::RGB(200, 200, 200)); // Adjusted y from 15 to 12

        // Draw exit button on the right with adjusted y position
        let exit_text = "EXIT";
        let exit_button_width = 45i32;
        let exit_button_height = 22i32;
        let exit_x = (SCREEN_WIDTH - 65) as i32;

        // Draw exit button background with adjusted y position
        let exit_button_rect = Rect::new(
            exit_x,
            7,
            exit_button_width as u32,
            exit_button_height as u32,
        ); // Adjusted y from 9 to 7
        self.screen_surface
            .fill_rect(exit_button_rect, Color::RGBA(180, 60, 60, 200))
            .unwrap();

        // Draw exit button border with adjusted y positions
        let border_rects = [
            Rect::new(exit_x, 7, exit_button_width as u32, 2), // Top
            Rect::new(
                exit_x,
                7 + exit_button_height - 2,
                exit_button_width as u32,
                2,
            ), // Bottom
            Rect::new(exit_x, 7, 2, exit_button_height as u32), // Left
            Rect::new(
                exit_x + exit_button_width - 2,
                7,
                2,
                exit_button_height as u32,
            ), // Right
        ];
        for border_rect in &border_rects {
            self.screen_surface
                .fill_rect(*border_rect, Color::RGB(220, 80, 80))
                .unwrap();
        }

        // Center the EXIT text within the button with adjusted y position
        let exit_text_width = exit_text.len() as i32 * 6;
        let exit_text_x = exit_x + (exit_button_width - exit_text_width) / 2;
        let exit_text_y = 7 + (exit_button_height - 7) / 2; // Adjusted y calculation
        self.draw_header_text(
            exit_text,
            exit_text_x,
            exit_text_y,
            Color::RGB(255, 255, 255),
        );
    }

    /// Gets the current time as a formatted string.
    ///
    /// # Returns
    ///
    /// Current time in HH:MM:SS format
    fn get_current_time_string(&self) -> String {
        let now = Local::now();
        now.format("%H:%M:%S").to_string()
    }

    /// Draws text on the header bar using simple pixel font.
    ///
    /// Uses a custom 5x7 bitmap font with 6-pixel character spacing.
    ///
    /// # Arguments
    ///
    /// * `text` - Text string to render
    /// * `x` - X position for text start
    /// * `y` - Y position for text baseline
    /// * `color` - Color for text rendering
    fn draw_header_text(&mut self, text: &str, x: i32, y: i32, color: Color) {
        for (i, ch) in text.chars().enumerate() {
            let char_x = x + (i as i32 * 6);
            self.draw_header_char(ch, char_x, y, color);
        }
    }

    /// Draws a single character using a simple 5x7 pixel font.
    ///
    /// Uses bitmap patterns for uppercase letters, digits, and common punctuation.
    /// Unsupported characters are rendered as 'O'.
    ///
    /// # Arguments
    ///
    /// * `ch` - Character to render (converted to uppercase)
    /// * `x` - X position for character
    /// * `y` - Y position for character
    /// * `color` - Color for character rendering
    fn draw_header_char(&mut self, ch: char, x: i32, y: i32, color: Color) {
        // Simple 5x7 bitmap font patterns
        let pattern = match ch.to_ascii_uppercase() {
            'A' => [
                0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'B' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
            ],
            'C' => [
                0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
            ],
            'D' => [
                0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
            ],
            'E' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
            'F' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'G' => [
                0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
            ],
            'H' => [
                0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'I' => [
                0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            'L' => [
                0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
            ],
            'M' => [
                0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001,
            ],
            'N' => [
                0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
            ],
            'O' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'P' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'R' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
            ],
            'S' => [
                0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            'T' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'U' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'V' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100,
            ],
            'W' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10101, 0b11011, 0b10001,
            ],
            'X' => [
                0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001,
            ],
            'Y' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'Z' => [
                0b11111, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000, 0b11111,
            ],
            '0' => [
                0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
            ],
            '1' => [
                0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            '2' => [
                0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
            ],
            '3' => [
                0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            '4' => [
                0b10010, 0b10010, 0b10010, 0b11111, 0b00010, 0b00010, 0b00010,
            ],
            '5' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
            ],
            '6' => [
                0b01111, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
            ],
            '7' => [
                0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
            ],
            '8' => [
                0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
            ],
            '9' => [
                0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b11110,
            ],
            ':' => [
                0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
            ],
            ' ' => [0; 7],
            '\'' => [
                0b01100, 0b01100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
            ],
            '.' => [
                0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
            ],
            '=' => [
                0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
            ],
            _ => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
        };

        // Draw the character pattern
        for (row, &line) in pattern.iter().enumerate() {
            for col in 0..5 {
                if (line >> (4 - col)) & 1 == 1 {
                    let pixel_rect = Rect::new(x + col, y + row as i32, 1, 1);
                    self.screen_surface.fill_rect(pixel_rect, color).unwrap();
                }
            }
        }
    }

    /// Renders the footer bar containing FPS counter and controls.
    ///
    /// The footer bar is a semi-transparent dark overlay at the bottom of the screen
    /// containing:
    /// - FPS counter on the left
    /// - Control mappings in the center (Z=B, X=A, etc.)
    ///
    /// The controls text has a darker background for better readability.
    fn render_footer_bar(&mut self) {
        let footer_height = 55u32; // Increased from 50 to 55
        let footer_y = SCREEN_HEIGHT - footer_height;
        let footer_rect = Rect::new(0, footer_y as i32, SCREEN_WIDTH, footer_height);

        // Draw semi-transparent dark background for footer
        self.screen_surface
            .fill_rect(footer_rect, Color::RGBA(0, 0, 0, 180))
            .unwrap();

        // Draw controls text centered in footer
        let controls_text = "Z=B  X=A  ARROWS=DPAD  ENTER=START  TAB=SELECT  ESC=EXIT";
        let text_width = controls_text.len() as i32 * 6; // 6 pixels per character
        let controls_x = (SCREEN_WIDTH as i32 - text_width) / 2;
        let controls_y = footer_y as i32 + 25; // Adjusted for new footer height

        // Draw controls background
        let bg_padding = 5;
        let bg_rect = Rect::new(
            controls_x - bg_padding,
            controls_y - bg_padding,
            (text_width + 2 * bg_padding) as u32,
            20, // Fixed height for controls background
        );
        self.screen_surface
            .fill_rect(bg_rect, Color::RGBA(0, 0, 0, 160))
            .unwrap();

        // Draw controls text
        self.draw_header_text(
            controls_text,
            controls_x,
            controls_y,
            Color::RGB(255, 255, 255),
        );

        // Draw FPS in bottom left corner at same level as controls
        let fps_text = format!("FPS: {}", self.fps_display);
        let fps_x = 20; // Consistent with header padding
        let fps_y = controls_y; // Same y level as controls
        self.draw_header_text(&fps_text, fps_x, fps_y, Color::RGB(255, 255, 255));
    }

    /// Updates audio by getting samples from the audio system and queuing them.
    ///
    /// Checks the audio queue size and adds samples if the buffer is getting low.
    /// Maintains a target buffer size of 4096 samples for smooth playback. Fills
    /// with silence if no samples are available from the APU.
    ///
    /// # Buffer Management
    ///
    /// - Target queue size: 4096 samples
    /// - Adds up to 1024 samples per call if queue is low
    /// - Automatically fills with silence when APU buffer is empty
    ///
    /// # Arguments
    ///
    /// * `cpu` - Mutable CPU reference for APU sample buffer access
    pub fn update_audio(&mut self, cpu: &mut CPU) {
        if let Some(ref audio_queue) = self.audio_queue {
            // Get available queue size
            let queue_size = audio_queue.size();
            let target_queue_size = 4096; // Keep a reasonable buffer

            // Add samples if queue is getting low
            if queue_size < target_queue_size {
                let samples_needed = (target_queue_size - queue_size).min(1024);
                let mut audio_buffer = vec![0.0f32; samples_needed as usize];

                // Get samples from the audio system
                let available_samples = cpu.bus.apu.sample_buffer.len();
                if available_samples > 0 {
                    // Get actual samples from the audio buffer
                    let copy_len = available_samples.min(samples_needed as usize);
                    cpu.bus.apu.get_samples(&mut audio_buffer[..copy_len]);

                    // Fill remaining with silence if needed
                    for i in copy_len..audio_buffer.len() {
                        audio_buffer[i] = 0.0;
                    }
                } else {
                    // If no samples available, fill with silence
                    for sample in audio_buffer.iter_mut() {
                        *sample = 0.0;
                    }
                }

                // Queue the audio samples using the non-deprecated method
                let _ = audio_queue.queue_audio(&audio_buffer);
            }
        }
    }

    pub fn process_events(&mut self, mut cpu_lock: MutexGuard<CPU>) -> bool {
        let mut should_continue = true;
        for event in self.event_pump.poll_iter() {
            match event {
                // Handle quit events (X button, Alt+F4, etc.)
                sdl2::event::Event::Quit { .. } => {
                    should_continue = false;
                }
                // Handle window close events
                sdl2::event::Event::Window {
                    win_event: sdl2::event::WindowEvent::Close,
                    ..
                } => {
                    should_continue = false;
                }
                // Handle key down events
                sdl2::event::Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => {
                    // Check for exit key first
                    if keycode == sdl2::keyboard::Keycode::Escape {
                        self.exit_requested = true;
                        should_continue = false;
                    } else {
                        // Handle game input
                        match keycode {
                            sdl2::keyboard::Keycode::Z => cpu_lock.bus.gamepad.state.b = true,
                            sdl2::keyboard::Keycode::X => cpu_lock.bus.gamepad.state.a = true,
                            sdl2::keyboard::Keycode::Return => {
                                cpu_lock.bus.gamepad.state.start = true
                            }
                            sdl2::keyboard::Keycode::Tab => {
                                cpu_lock.bus.gamepad.state.select = true
                            }
                            sdl2::keyboard::Keycode::Up => cpu_lock.bus.gamepad.state.up = true,
                            sdl2::keyboard::Keycode::Down => cpu_lock.bus.gamepad.state.down = true,
                            sdl2::keyboard::Keycode::Left => cpu_lock.bus.gamepad.state.left = true,
                            sdl2::keyboard::Keycode::Right => {
                                cpu_lock.bus.gamepad.state.right = true
                            }
                            _ => {}
                        }
                    }
                }
                // Handle key up events
                sdl2::event::Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    // Handle game input
                    match keycode {
                        sdl2::keyboard::Keycode::Z => cpu_lock.bus.gamepad.state.b = false,
                        sdl2::keyboard::Keycode::X => cpu_lock.bus.gamepad.state.a = false,
                        sdl2::keyboard::Keycode::Return => cpu_lock.bus.gamepad.state.start = false,
                        sdl2::keyboard::Keycode::Tab => cpu_lock.bus.gamepad.state.select = false,
                        sdl2::keyboard::Keycode::Up => cpu_lock.bus.gamepad.state.up = false,
                        sdl2::keyboard::Keycode::Down => cpu_lock.bus.gamepad.state.down = false,
                        sdl2::keyboard::Keycode::Left => cpu_lock.bus.gamepad.state.left = false,
                        sdl2::keyboard::Keycode::Right => cpu_lock.bus.gamepad.state.right = false,
                        _ => {}
                    }
                }
                // Handle mouse button clicks for EXIT button
                sdl2::event::Event::MouseButtonDown {
                    mouse_btn: sdl2::mouse::MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    // Check exit button click
                    let exit_x = (SCREEN_WIDTH - 55) as i32;
                    let exit_button_width = 45i32;
                    let exit_button_height = 22i32;

                    let clicked_exit = self.show_header
                        && x >= exit_x
                        && x < exit_x + exit_button_width
                        && y >= 4
                        && y < 4 + exit_button_height;

                    if clicked_exit {
                        self.exit_requested = true;
                        should_continue = false;
                    }
                }
                _ => {}
            }
        }

        // Update audio while we have the CPU lock
        self.update_audio(&mut cpu_lock);

        should_continue
    }
}

/// Cross-platform delay function using standard library sleep.
///
/// Pauses execution for the specified number of milliseconds. Used for
/// frame rate limiting and timing control.
///
/// # Arguments
///
/// * `ms` - Number of milliseconds to delay
pub fn delay(ms: u32) {
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
}

/// Gets current time in milliseconds since Unix epoch.
///
/// Used for frame timing and FPS calculations. Provides a monotonic
/// timestamp for measuring elapsed time.
///
/// # Returns
///
/// Current time in milliseconds since January 1, 1970 00:00:00 UTC
pub fn get_ticks() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
