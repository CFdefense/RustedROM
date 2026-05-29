/*
  main.rs
  Info: Main entry point for RustedROM Game Boy Emulator
  Description: The main module implements the application entry point and menu system initialization.
              Handles command line argument parsing, ROM scanning, and launches the game selection interface.

  Main Function:
    main: Entry point - Initializes menu system with debug mode support and starts the game selection loop

  Module Functions:
    launch_emulator: Game Launcher - Starts the emulator for a specific ROM file using existing UI context
    main_direct_rom: Direct ROM Mode - Backwards compatibility function for direct ROM loading (unused in menu mode)

  Key Features:
    - Command line argument parsing for --debug mode
    - Automatic ROM scanning in the "roms" directory
    - Menu-driven game selection interface
    - Game launching with existing UI context reuse
    - Clean shutdown and return to menu after game sessions
    - Debug mode propagation throughout the system

  Dependencies:
    - MenuContext: Game selection state management
    - MenuState: Current menu navigation state
    - GameScanner: ROM file discovery and metadata extraction
    - MenuRenderer: Menu display and user interface rendering
    - UI: SDL2-based graphics and input handling
    - emu: Core emulation engine integration

  Program Flow:
    1. Parse command line arguments (--debug flag)
    2. Initialize menu context with debug settings
    3. Scan "roms" directory for Game Boy ROMs
    4. Enter main menu loop with keyboard navigation
    5. Launch selected games in emulator context
    6. Return to menu after game sessions end
    7. Clean shutdown on exit request
*/

use std::env;
use std::time::Instant;

mod hdw;
mod menu;

use hdw::ui::UI;
use menu::{GameScanner, MenuContext, MenuRenderer, MenuState};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

fn main() -> Result<(), String> {
    println!("RustedROM - Game Boy Emulator");
    println!("=============================");

    // Parse command line arguments for debug mode
    let args: Vec<String> = env::args().collect();
    let debug = args.contains(&"--debug".to_string());

    if debug {
        println!("Debug mode enabled");
    }

    // Initialize menu system
    let mut menu_context = MenuContext::new(debug);

    // Scan for games
    println!("Scanning for Game Boy ROMs...");
    menu_context.games = GameScanner::scan_games("roms");
    println!("Found {} games", menu_context.games.len());

    // Initialize UI for menu
    let mut ui = UI::new(debug)?; // Pass debug flag to enable debug window for menu
    let mut last_time = Instant::now();

    // Main application loop
    loop {
        let current_time = Instant::now();
        let delta_time = (current_time - last_time).as_secs_f32();
        last_time = current_time;

        // Update menu context
        menu_context.update(delta_time);

        // Handle menu events
        let mut continue_running = true;
        let mut launch_game: Option<String> = None;

        for event in ui.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    continue_running = false;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Up => menu_context.navigate_up(),
                    Keycode::Down => menu_context.navigate_down(),
                    Keycode::Left | Keycode::Right => {
                        if matches!(menu_context.current_state, MenuState::GameSelection) {
                            menu_context.switch_tab();
                        }
                    }
                    Keycode::Return => {
                        if let Some(game_path) = menu_context.select() {
                            launch_game = Some(game_path);
                        }
                    }
                    Keycode::Backspace => menu_context.back(),
                    Keycode::Escape => {
                        if matches!(menu_context.current_state, MenuState::InGame(_)) {
                            menu_context.exit_game();
                        } else {
                            continue_running = false;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if !continue_running {
            break;
        }

        // Launch game if requested
        if let Some(game_path) = launch_game {
            println!("Launching game: {}", game_path);
            let palette_colors = menu_context.get_current_palette().get_colors();
            match launch_emulator(
                &game_path,
                &mut ui,
                menu_context.debug,
                Some(palette_colors),
            ) {
                Ok(_) => {
                    println!("Game session ended, returning to menu");
                    menu_context.exit_game();
                }
                Err(e) => {
                    println!("Failed to launch game: {}", e);
                    menu_context.exit_game();
                }
            }
        }

        // Render menu (only if not in game)
        if !matches!(menu_context.current_state, MenuState::InGame(_)) {
            MenuRenderer::render_menu(
                &mut ui.screen_surface,
                &menu_context,
                hdw::ui::SCREEN_WIDTH,
                hdw::ui::SCREEN_HEIGHT,
            );

            // Create texture and render to main window
            let main_texture = ui
                .main_texture_creator
                .create_texture_from_surface(&ui.screen_surface)
                .expect("Failed to create main texture");

            ui.main_canvas.clear();
            ui.main_canvas.copy(&main_texture, None, None).unwrap();
            ui.main_canvas.present();
        }

        // Small delay to prevent high CPU usage
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }

    println!("Thanks for using RustedROM!");
    Ok(())
}

fn launch_emulator(
    rom_path: &str,
    ui: &mut UI,
    debug: bool,
    palette: Option<[u32; 4]>,
) -> Result<(), String> {
    println!("Starting Game Boy emulator for: {}", rom_path);

    // Use the new function that accepts an existing UI context
    match hdw::emu::emu_run_with_ui(rom_path, ui, None, debug, palette) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Emulator error: {}", e)),
    }
}
