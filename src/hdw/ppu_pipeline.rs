/**
 * PPU Pipeline Module - Game Boy Pixel FIFO Implementation
 *
 * This module implements the Game Boy's pixel pipeline using a First-In-First-Out (FIFO)
 * buffer system that accurately replicates the original hardware's pixel processing.
 * The pipeline fetches tile data, processes background/window/sprite pixels, and outputs
 * the final color values that get displayed on screen.
 *
 * Pipeline Stages:
 * 1. TILE: Fetch tile number from background/window map
 * 2. DATA0: Fetch low bit plane of tile data  
 * 3. DATA1: Fetch high bit plane of tile data
 * 4. IDLE: Wait state for timing accuracy
 * 5. PUSH: Push 8 pixels into FIFO for rendering
 *
 * FIFO Operation:
 * The pixel FIFO maintains a queue of up to 16 pixels, with new pixels pushed
 * from the back and rendered pixels popped from the front. This creates the
 * authentic timing behavior needed for proper scrolling and sprite mixing.
 *
 * Background/Window Processing:
 * - Fetches 8x8 tile data from VRAM based on tile maps
 * - Handles both 8000-8FFF and 8800-97FF tile data addressing modes
 * - Supports horizontal and vertical scrolling through SCX/SCY registers
 * - Window layer can override background tiles based on WX/WY positioning
 *
 * Sprite Integration:
 * - Up to 3 sprites can be processed simultaneously during pixel fetch
 * - Sprite pixels are mixed with background pixels based on priority flags
 * - Supports both 8x8 and 8x16 sprite modes with proper clipping
 *
 * The pipeline ensures cycle-accurate pixel output timing for proper game compatibility.
 */

/**
 * FIFOState - Pixel Pipeline State Machine
 *
 * Represents the current stage of the pixel fetching pipeline.
 * Each state corresponds to a specific operation in the tile data fetch process.
 */
pub enum FIFOState {
    /// Fetch tile number from background/window tile map
    TILE,
    /// Fetch low bit plane (bits 0) of tile data
    DATA0,
    /// Fetch high bit plane (bits 1) of tile data  
    DATA1,
    /// Idle state for timing synchronization
    IDLE,
    /// Push 8 processed pixels into FIFO queue
    PUSH,
}

/**
 * FIFO - First-In-First-Out Pixel Buffer
 *
 * Maintains a queue of processed pixels waiting to be rendered.
 * Implements the Game Boy's authentic pixel timing behavior.
 */
pub struct FIFO {
    /// Vector storing 32-bit ARGB pixel values
    pub entries: Vec<u32>,
    /// Maximum number of pixels that can be buffered
    pub max_size: usize,
}

impl FIFO {
    pub fn new() -> Self {
        FIFO {
            entries: Vec::new(),
            max_size: 10,
        }
    }
}

/**
 * PixelFIFO - Complete Pixel Processing Pipeline
 *
 * Combines the FIFO buffer with all state needed for pixel processing.
 * Manages tile fetching, coordinate tracking, and pixel data storage
 * for both background/window and sprite rendering.
 *
 * Coordinate System:
 * - line_x: Current X position being processed on scanline
 * - pushed_x: Number of pixels output to display buffer
 * - fetch_x: X position being fetched (8-pixel aligned)
 * - map_x/map_y: Background/window tile map coordinates
 * - tile_y: Y offset within current tile (0-7, doubled for bit planes)
 */
pub struct PixelFIFO {
    pub state: FIFOState,
    pub fifo: FIFO,
    pub line_x: u8,
    pub pushed_x: u8,
    pub fetch_x: u8,
    pub bgw_fetch_data: [u8; 3],
    pub fetch_entry_data: [u8; 6],
    pub map_x: u8,
    pub map_y: u8,
    pub tile_y: u8,
    pub fifo_x: u8,
}

impl PixelFIFO {
    pub fn new() -> Self {
        PixelFIFO {
            state: FIFOState::TILE,
            fifo: FIFO::new(),
            line_x: 0,
            pushed_x: 0,
            fetch_x: 0,
            bgw_fetch_data: [0; 3],
            fetch_entry_data: [0; 6],
            map_x: 0,
            map_y: 0,
            tile_y: 0,
            fifo_x: 0,
        }
    }

    pub fn pixel_fifo_push(&mut self, value: u32) {
        if self.fifo.entries.len() < self.fifo.max_size {
            self.fifo.entries.push(value);
        }
    }

    pub fn pixel_fifo_pop(&mut self) -> Option<u32> {
        if self.fifo.entries.is_empty() {
            None
        } else {
            Some(self.fifo.entries.remove(0))
        }
    }

    pub fn fifo_size(&self) -> usize {
        self.fifo.entries.len()
    }

    pub fn pipeline_fifo_reset(&mut self) {
        // Pop all entries from the FIFO
        while self.fifo_size() > 0 {
            self.pixel_fifo_pop();
        }
        self.fifo.entries.clear();
    }
}
