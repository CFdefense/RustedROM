/*
  hdw/emu.rs
  Info: Core emulation engine and timing coordination system
  Description: The emu module implements the central emulation context and timing synchronization.
              Manages system-wide state, coordinates hardware component timing, and provides
              the main emulation loop with accurate Game Boy timing characteristics.

  EmuContext Struct Members:
    running: Emulation State - Controls whether the emulation loop continues execution
    paused: Pause State - Temporarily halts execution while maintaining state
    die: Shutdown Flag - Signals complete emulation shutdown and cleanup
    ticks: Cycle Counter - Global T-cycle counter for accurate timing synchronization
    cpu: CPU Reference - Thread-safe reference to the CPU for cross-thread access
    debug_limit: Debug Limit - Optional instruction count limit for debugging sessions
    instruction_count: Instruction Counter - Tracks executed instructions for debug limits
    timer: System Timer - Hardware timer component for time-based interrupts
    debug: Debug Mode - Global debug flag propagated throughout the system

  Core Functions:
    EmuContext::new: Constructor - Creates new emulation context with timing and debug settings
    init_global_emu_context: Global Setup - Initializes system-wide emulation context reference
    cpu_run: CPU Thread - Main CPU execution loop running in dedicated thread
    emu_run: CLI Entry Point - Command-line interface for direct ROM loading (legacy mode)
    emu_run_with_ui: UI Integration - Emulation with full UI and menu system integration
    emu_cycles: Timing Engine - Increments system timing and coordinates hardware updates
    is_debug_enabled: Debug Check - Global debug mode state accessor

  Timing Architecture:
    - T-cycle based timing (4 T-cycles = 1 M-cycle) matching original Game Boy
    - Each T-cycle updates timer, PPU, audio, and DMA components
    - Synchronized hardware component ticking for accurate emulation
    - Interrupt handling coordinated through cycle-accurate timing
    - Frame-rate regulation through PPU frame counter tracking

  Threading Model:
    - CPU execution runs in dedicated thread for performance
    - UI/input handling runs in main thread for responsiveness
    - Thread-safe communication through Arc<Mutex<>> wrappers
    - Global context provides safe cross-thread state access
    - Clean thread shutdown coordination on emulation exit

  Memory and Hardware Coordination:
    - Bus interface connects all hardware components
    - PPU generates V-blank and LCD status interrupts
    - Timer generates timer overflow interrupts
    - Audio system runs independently with T-cycle accuracy
    - DMA transfers coordinated with CPU execution

  Debug Integration:
    - Optional instruction count limits for development
    - Debug mode propagation to all system components
    - Performance monitoring through cycle counting
    - State inspection capabilities for debugging
    - Logging coordination across hardware modules

  Game Integration:
    - ROM loading and cartridge initialization
    - Game name extraction for UI display
    - Battery save coordination for persistent data
    - Input mapping from UI to gamepad controller
    - Display output routing from PPU to UI system

  Error Handling:
    - Graceful degradation on component failures
    - Thread panic recovery mechanisms
    - Clean shutdown procedures for all components
    - Debug logging for error diagnosis
    - Safe state preservation during errors
*/

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Import your required modules
use crate::hdw::bus::BUS;
use crate::hdw::cart::Cartridge;
use crate::hdw::cpu::CPU;
use crate::hdw::timer::Timer;
use crate::hdw::ui::UI;

use once_cell::sync::OnceCell;

// Global static EmuContext holder
pub static EMU_CONTEXT: OnceCell<Arc<Mutex<EmuContext>>> = OnceCell::new();

// Emulator context
pub struct EmuContext {
    pub running: bool,
    pub paused: bool,
    pub die: bool,
    pub ticks: u64,
    pub cpu: Option<Arc<Mutex<CPU>>>,
    debug_limit: Option<u32>,
    instruction_count: u32,
    pub timer: Timer,
    pub debug: bool,
}

impl EmuContext {
    pub fn new(debug_limit: Option<u32>, debug: bool) -> Self {
        EmuContext {
            running: false,
            paused: false,
            die: false,
            ticks: 0,
            cpu: None,
            debug_limit,
            instruction_count: 0,
            timer: Timer::new(),
            debug,
        }
    }
}

// Function to initialize the global EmuContext reference.
// This should be called once from emu_run after ctx is created and configured.
pub fn init_global_emu_context(ctx: Arc<Mutex<EmuContext>>) {
    // Attempt to set the context. If it's already set, .set() will return an Err.
    // We are choosing to do nothing if it's already initialized.
    let _ = EMU_CONTEXT.set(ctx);
}

// CPU thread function
fn cpu_run(cpu: Arc<Mutex<CPU>>, ctx: Arc<Mutex<EmuContext>>) {
    while ctx.lock().unwrap().running {
        if ctx.lock().unwrap().paused {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // Execute a CPU step
        let result = {
            let mut cpu_lock = cpu.lock().unwrap();
            cpu_lock.step(Arc::clone(&ctx)) // Pass a clone of the Arc to step
        };

        if !result {
            println!("CPU Stopped");
            ctx.lock().unwrap().running = false;
            break;
        }

        // Update instruction count and check debug limit
        {
            let mut ctx_lock = ctx.lock().unwrap();
            ctx_lock.instruction_count += 1;

            if let Some(limit) = ctx_lock.debug_limit {
                if ctx_lock.instruction_count >= limit {
                    println!("\nDebug limit of {} instructions reached. Stopping.", limit);
                    ctx_lock.running = false;
                    break;
                }
            }
        }
    }
}

// Main Emulator Startup Function
#[allow(dead_code)]
pub fn emu_run(args: Vec<String>) -> io::Result<()> {
    // Parse command line arguments
    let mut rom_path = None;
    let mut debug_limit = None;
    let mut debug = false;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--debug-limit" => {
                if i + 1 < args.len() {
                    debug_limit = Some(args[i + 1].parse().expect("Debug limit must be a number"));
                    i += 2;
                    continue;
                }
            }
            "--debug" => {
                debug = true;
                i += 1;
                continue;
            }
            path => {
                rom_path = Some(path);
                i += 1;
            }
        }
    }

    // Check if ROM path was provided
    let rom_path = rom_path
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing ROM file argument"))?;

    // Initialize UI
    let ui_result = UI::new(debug);
    if let Err(e) = &ui_result {
        println!("Failed to initialize UI: {}", e);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to initialize UI: {}", e),
        ));
    }
    let mut ui = ui_result.unwrap();

    emu_run_with_ui(rom_path, &mut ui, debug_limit, debug, None)
}

pub fn emu_run_with_ui(
    rom_path: &str,
    ui: &mut UI,
    debug_limit: Option<u32>,
    debug: bool,
    palette: Option<[u32; 4]>,
) -> io::Result<()> {
    // Attempt to create Cartridge
    let mut cart = Cartridge::new();
    if let Err(e) = cart.load_cart(rom_path) {
        println!("Failed to load ROM file: {}", e);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to load ROM file: {}", e),
        ));
    }
    println!("Cart loaded..");

    // Extract game name from ROM path and set it in UI
    let game_name = std::path::Path::new(rom_path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    ui.set_game_name(game_name);
    ui.show_header = true;
    ui.exit_requested = false;

    // Initialize context first
    let ctx = Arc::new(Mutex::new(EmuContext::new(debug_limit, debug)));

    // Initialize Bus and CPU
    let mut bus = BUS::new();
    bus.cart = cart;
    let cpu = Arc::new(Mutex::new(CPU::new(bus, debug)));

    // Apply palette if provided
    if let Some(palette_colors) = palette {
        if let Ok(mut cpu_lock) = cpu.lock() {
            cpu_lock.bus.ppu.lcd.update_default_colors(palette_colors);
        }
    }

    // Update context with CPU
    {
        let mut ctx_lock = ctx.lock().unwrap();
        ctx_lock.cpu = Some(Arc::clone(&cpu));
        ctx_lock.running = true;
    }

    // Initialize the global context reference
    init_global_emu_context(Arc::clone(&ctx));

    // Spawn a new thread for CPU execution
    let cpu_thread_ctx = Arc::clone(&ctx);
    let cpu_thread_cpu = Arc::clone(&cpu);

    let cpu_thread = thread::spawn(move || {
        cpu_run(cpu_thread_cpu, cpu_thread_ctx);
    });

    // Main loop for UI and event handling
    let mut prev_frame = 0;

    while !{
        let ctx_lock_result = ctx.lock();
        match ctx_lock_result {
            Ok(ctx_lock) => ctx_lock.die,
            Err(_) => {
                println!("Context mutex poisoned, shutting down");
                true
            }
        }
    } && !ui.exit_requested
    {
        // Small delay
        thread::sleep(Duration::from_millis(1));

        // Handle UI events without holding CPU lock
        let continue_running = {
            let cpu_lock_result = cpu.lock();
            let mut cpu_lock = match cpu_lock_result {
                Ok(lock) => lock,
                Err(_) => {
                    println!("CPU mutex poisoned, shutting down");
                    break;
                }
            };

            // Process events first (without calling ui_update)
            let mut should_continue = true;
            for event in ui.event_pump.poll_iter() {
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
                            ui.exit_requested = true;
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
                                sdl2::keyboard::Keycode::Down => {
                                    cpu_lock.bus.gamepad.state.down = true
                                }
                                sdl2::keyboard::Keycode::Left => {
                                    cpu_lock.bus.gamepad.state.left = true
                                }
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
                            sdl2::keyboard::Keycode::Return => {
                                cpu_lock.bus.gamepad.state.start = false
                            }
                            sdl2::keyboard::Keycode::Tab => {
                                cpu_lock.bus.gamepad.state.select = false
                            }
                            sdl2::keyboard::Keycode::Up => cpu_lock.bus.gamepad.state.up = false,
                            sdl2::keyboard::Keycode::Down => {
                                cpu_lock.bus.gamepad.state.down = false
                            }
                            sdl2::keyboard::Keycode::Left => {
                                cpu_lock.bus.gamepad.state.left = false
                            }
                            sdl2::keyboard::Keycode::Right => {
                                cpu_lock.bus.gamepad.state.right = false
                            }
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
                        let exit_x = (crate::hdw::ui::SCREEN_WIDTH - 55) as i32;
                        let exit_button_width = 45i32;
                        let exit_button_height = 22i32;

                        let clicked_exit = ui.show_header
                            && x >= exit_x
                            && x < exit_x + exit_button_width
                            && y >= 4
                            && y < 4 + exit_button_height;

                        if clicked_exit {
                            ui.exit_requested = true;
                            should_continue = false;
                        }
                    }
                    _ => {}
                }
            }

            // Update audio while we have the CPU lock
            ui.update_audio(&mut cpu_lock);

            should_continue
        };

        // Now update UI without holding CPU lock
        {
            let cpu_lock_result = cpu.lock();
            let mut cpu_lock = match cpu_lock_result {
                Ok(lock) => lock,
                Err(_) => {
                    println!("CPU mutex poisoned during UI update, shutting down");
                    break;
                }
            };

            // Check if frame has changed and update UI
            let current_frame = cpu_lock.bus.ppu.current_frame;
            if prev_frame != current_frame {
                ui.ui_update(&mut cpu_lock);
                prev_frame = current_frame;
            }
        }

        if !continue_running || ui.exit_requested {
            println!("UI requested shutdown");
            if let Ok(mut ctx_lock) = ctx.lock() {
                ctx_lock.die = true;
                ctx_lock.running = false;
            }
            break;
        }
    }

    // Disable header when exiting game
    ui.show_header = false;
    ui.current_game_name = None;

    // Wait for CPU thread to finish
    if let Err(e) = cpu_thread.join() {
        println!("Error joining CPU thread: {:?}", e);
    }

    // Make sure to properly signal the CPU thread to stop
    if let Ok(mut ctx_lock) = ctx.lock() {
        ctx_lock.running = false;
    }

    Ok(())
}

// Function to increment EmuContext ticks based on CPU M-cycles.
// Each M-cycle is typically 4 T-cycles (clock ticks).
// CPU reference is passed directly to avoid double-locking issues.
pub fn emu_cycles(cpu: &mut CPU, cpu_m_cycles: u8) {
    if let Some(ctx_arc) = EMU_CONTEXT.get() {
        let t_cycles_to_add = cpu_m_cycles as u64 * 4; // Calculate total T-cycles to add
        if let Ok(mut emu_ctx_lock) = ctx_arc.lock() {
            for _ in 0..t_cycles_to_add {
                emu_ctx_lock.ticks += 1;
                // Call timer_tick with the passed CPU reference
                emu_ctx_lock.timer.timer_tick(cpu);
                // Tick PPU for every T-cycle and handle interrupts
                let ppu_interrupts = cpu.bus.ppu.ppu_tick(&mut cpu.bus.cart);
                for interrupt in ppu_interrupts {
                    cpu.bus.interrupt_controller.request_interrupt(interrupt);
                }
                // Tick audio for every T-cycle
                cpu.bus.apu.tick();
            }
            // Update LCD LY register from PPU
            cpu.bus.ppu.update_lcd_ly();

            // Release the lock before ticking DMA to avoid deadlock
            drop(emu_ctx_lock);

            // Tick DMA on the CPU's bus (where the game actually runs)
            cpu.bus.tick_dma(); // tick once per 4 t-cycles
        } else {
            eprintln!("emu_cycles: Failed to lock EmuContext.");
        }
    } else {
        panic!(
            "emu_cycles: Global EmuContext not initialized. Call init_global_emu_context first."
        );
    }
}

pub fn is_debug_enabled() -> bool {
    if let Some(ctx_arc) = EMU_CONTEXT.get() {
        if let Ok(ctx_lock) = ctx_arc.lock() {
            return ctx_lock.debug;
        }
    }
    false
}
