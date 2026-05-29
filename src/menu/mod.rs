/*
  menu/mod.rs
  Info: Menu system module declarations for game selection interface
  Description: The menu module implements a complete game selection and navigation system.
              Provides a user-friendly interface for browsing, selecting, and launching Game Boy ROMs
              with visual previews and game information display.

  Menu Components:
    menu_state: State Management - Menu navigation state, game selection, and UI mode tracking
    menu_renderer: Display System - SDL2-based rendering with custom bitmap fonts and image support
    game_scanner: ROM Discovery - Automatic scanning and metadata extraction from ROM files

  Key Features:
    - Automatic ROM detection in "roms" directory
    - Visual game previews with image loading support
    - Game metadata display (size, battery save support, cartridge type)
    - Keyboard navigation with arrow keys and Enter/Backspace
    - Scrolling game list for large ROM collections
    - Game launch with seamless emulator integration
    - Return to menu after game sessions
    - Debug mode support for development

  Menu States:
    - MainMenu: Initial screen with START and CREDITS options
    - GameSelection: ROM browser with preview pane and game information
    - Credits: Information about the emulator and its features
    - InGame: Active emulation session (menu hidden)

  Integration:
    - Seamlessly launches emulation sessions with selected ROMs
    - Maintains UI context between menu and emulation modes
    - Supports debug mode propagation to emulation engine
    - Handles clean shutdown and return to menu after games

  User Experience:
    - Split-screen layout with game list and preview/info panels
    - Real-time image loading and scaling for game previews
    - Keyboard shortcuts for efficient navigation
    - Visual feedback for selections and state changes
    - Consistent theming with Game Boy-inspired color scheme
*/

pub mod color_palette;
pub mod menu;
pub mod menu_renderer;
pub mod rom_catalog;

// Re-export main types for easy access
pub use menu_renderer::*;
pub use rom_catalog::*;
pub use menu::*;
pub use color_palette::*;
