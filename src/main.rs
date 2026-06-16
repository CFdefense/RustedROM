// Main entry point for RustedROM Game Boy Emulator.
//
// The main module implements the application entry point and menu system initialization.
// Handles command line argument parsing, ROM scanning, and launches the game selection interface.
//
// Main Function:
//   main: Entry point - Initializes menu system with debug mode support and starts the game selection loop
//
// Module Functions:
//   launch_emulator: Game Launcher - Starts the emulator for a specific ROM file using existing UI context
//   main_direct_rom: Direct ROM Mode - Backwards compatibility function for direct ROM loading (unused in menu mode)
//
// Key Features:
//   - Command line argument parsing for --debug mode
//   - Automatic ROM scanning in the "roms" directory
//   - Menu-driven game selection interface
//   - Game launching with existing UI context reuse
//   - Clean shutdown and return to menu after game sessions
//   - Debug mode propagation throughout the system
//
// Dependencies:
//   - MenuContext: Game selection state management
//   - MenuState: Current menu navigation state
//   - GameScanner: ROM file discovery and metadata extraction
//   - MenuRenderer: Menu display and user interface rendering
//   - UI: SDL2-based graphics and input handling
//   - emu: Core emulation engine integration
//
// Program Flow:
//   1. Parse command line arguments (--debug flag)
//   2. Initialize menu context with debug settings
//   3. Scan "roms" directory for Game Boy ROMs
//   4. Enter main menu loop with keyboard navigation
//   5. Launch selected games in emulator context
//   6. Return to menu after game sessions end
//   7. Clean shutdown on exit request

use std::env;
use std::time::Instant;
mod hdw;
mod menu;

use hdw::emu::Emulator;
use menu::{MenuState};
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

    // Initialize Emulator
    let mut emu = Emulator::new(debug);
    let mut last_time = Instant::now();

    // Main application loop
    'app_loop: loop {
        let current_time = Instant::now();
        let delta_time = (current_time - last_time).as_secs_f32();
        last_time = current_time;

        // Update menu context
        emu.menu.update(delta_time);

        // match keybaord events to changes in menu
        for event in emu.ui.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'app_loop,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Return => emu.menu.select(),
                    Keycode::Escape | Keycode::Backspace => emu.menu.back(),
                    Keycode::Up => emu.menu.navigate_up(),
                    Keycode::Down => emu.menu.navigate_down(),
                    Keycode::Left | Keycode::Right => {
                        if matches!(emu.menu.current_state, MenuState::ROMSelection(_)) {
                            emu.menu.navigate_horizontal();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        // based on final current state -> render menu or ROM
        match &emu.menu.current_state {
            MenuState::ROMOpen(rom) => {
                // Set the palette properly
                emu.palette = Some(emu.menu.current_palette.clone());

                // Launch ROM if requested
                println!("Launching ROM: {}", rom.path);
                match emu.run(rom.path.clone()) {
                    Ok(_) => {
                        println!("ROM session ended, returning to menu");
                        emu.menu.back()
                    }
                    Err(e) => {
                        println!("Failed to launch game: {}", e);
                        emu.menu.back();
                    }
                }
            }
            _ => {
                // If not in game -> render menu
                emu.menu.render(&mut emu.ui.screen_surface);

                // Create texture and render to main window
                let main_texture = emu.ui
                    .main_texture_creator
                    .create_texture_from_surface(&emu.ui.screen_surface)
                    .expect("Failed to create main texture");

                emu.ui.main_canvas.clear();
                emu.ui.main_canvas.copy(&main_texture, None, None).unwrap();
                emu.ui.main_canvas.present();
            }
        }

        // Small delay to prevent high CPU usage
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }

    println!("Thanks for using RustedROM!");
    Ok(())
}
