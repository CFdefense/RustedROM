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
    instruction_count: Instruction Counter - Tracks executed instructions for debugging
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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::{io, path::Path};

use crate::hdw::bus::BUS;
use crate::hdw::cart::Cartridge;
use crate::hdw::cpu::CPU;
use crate::hdw::timer::Timer;
use crate::hdw::ui::{self, UI};
use crate::menu::{ColorPalette, Menu};

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
    instruction_count: u32,
    pub timer: Timer,
    pub debug: bool,
}

impl EmuContext {
    pub fn new(debug: bool) -> Self {
        Self {
            running: false,
            paused: false,
            die: false,
            ticks: 0,
            cpu: None,
            instruction_count: 0,
            timer: Timer::new(),
            debug,
        }
    }
}

pub struct Emulator {
    pub ui: UI,
    pub menu: Menu,
    debug: bool,
    pub palette: Option<ColorPalette>,
}

impl Emulator {
    pub fn new(debug: bool) -> Self {
        Self {
            ui: UI::new(debug).unwrap(),
            menu: Menu::new(
                PathBuf::from("roms"),
                ui::SCREEN_WIDTH,
                ui::SCREEN_WIDTH,
                debug,
            ),
            debug: debug,
            palette: None,
        }
    }

    pub fn run(&mut self, rom_path: String) -> io::Result<()> {
        // Attempt to load Cartridge
        let mut cart = Cartridge::new();
        if let Err(e) = cart.load_cart(&rom_path) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to load ROM file: {}", e),
            ));
        }
        println!("Cart loaded..");

        // Extract game name from ROM path and set it in UI
        let game_name = Path::new(&rom_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        self.ui.set_game_name(game_name);
        self.ui.show_header = true;
        self.ui.exit_requested = false;

        // Initialize context first
        let ctx = Arc::new(Mutex::new(EmuContext::new(self.debug)));

        // Initialize Bus and CPU
        let mut bus = BUS::new();
        bus.cart = cart;
        let cpu = Arc::new(Mutex::new(CPU::new(bus, self.debug)));

        // Apply palette if provided
        if let Some(palette_colors) = &self.palette {
            if let Ok(mut cpu_lock) = cpu.lock() {
                cpu_lock
                    .bus
                    .ppu
                    .lcd
                    .update_default_colors(palette_colors.get_colors());
            }
        }

        // Update context with CPU
        {
            let mut ctx_lock = ctx.lock().unwrap();
            ctx_lock.cpu = Some(Arc::clone(&cpu));
            ctx_lock.running = true;
        }

        // Initialize the global context reference
        // OnceCell only allows this to be set() once
        // Subsequent calls will Err
        let _ = EMU_CONTEXT.set(Arc::clone(&ctx));

        // Spawn a new thread for CPU execution
        let cpu_thread_ctx = Arc::clone(&ctx);
        let cpu_thread_cpu = Arc::clone(&cpu);

        let cpu_thread = thread::spawn(move || {
            cpu_run(cpu_thread_cpu, cpu_thread_ctx);
        });

        // Main loop for UI and event handling
        let mut prev_frame = 0;

        // Run event loop
        while !{
            let ctx_lock_result = ctx.lock();
            match ctx_lock_result {
                Ok(ctx_lock) => ctx_lock.die,
                Err(_) => {
                    println!("Context mutex poisoned, shutting down");
                    true
                }
            }
        } && !self.ui.exit_requested
        {
            // Small delay
            thread::sleep(Duration::from_millis(1));

            // Handle UI events without holding CPU lock
            let continue_running = {
                let cpu_lock_result = cpu.lock();
                let cpu_lock = match cpu_lock_result {
                    Ok(lock) => lock,
                    Err(_) => {
                        println!("CPU mutex poisoned, shutting down");
                        break;
                    }
                };

                self.ui.process_events(cpu_lock)
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
                    self.ui.ui_update(&mut cpu_lock);
                    prev_frame = current_frame;
                }
            }

            if !continue_running || self.ui.exit_requested {
                println!("UI requested shutdown");
                if let Ok(mut ctx_lock) = ctx.lock() {
                    ctx_lock.die = true;
                    ctx_lock.running = false;
                }
                break;
            }
        }

        // Disable header when exiting game
        self.ui.show_header = false;
        self.ui.current_game_name = None;

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
}

// CPU thread function
//
// This is the function which will run in a dedicated thread for cpu execution
//
// Later -> Migrate away from Mutex CPU into channel based approach
//
fn cpu_run(cpu: Arc<Mutex<CPU>>, ctx: Arc<Mutex<EmuContext>>) {
    // Aquire the ctx lock and check if running
    while ctx.lock().unwrap().running {
        // if paused we simply sleep and continue
        if ctx.lock().unwrap().paused {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // Execute a CPU step
        // May not need to pass ctx here in the future as im only using it for debug
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
        }
    }
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
