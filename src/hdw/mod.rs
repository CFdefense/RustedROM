// Hardware emulation module declarations for Game Boy components.
//
// The hdw module contains all hardware emulation components that collectively implement
// a complete Game Boy system. Each sub-module represents a specific hardware component
// with accurate timing and behavior emulation.
//
// Hardware Components:
//   cpu: Central Processing Unit - Sharp LR35902 processor with custom instruction set
//   bus: Memory Management Unit - Handles memory mapping and I/O routing between components
//   cart: Cartridge Interface - MBC1, MBC2, MBC3 memory bank controllers with battery save support
//   ram: Random Access Memory - Work RAM and High RAM (WRAM/HRAM) management
//   ppu: Picture Processing Unit - Graphics rendering with sprites, backgrounds, and window layers
//   apu: Audio Processing Unit - 4-channel sound synthesis (pulse, wave, noise)
//   lcd: LCD Controller - Display timing, modes, and register management
//   gamepad: Input Controller - Joypad input handling and button state management
//   timer: System Timer - Programmable timer with interrupt generation
//   dma: Direct Memory Access - High-speed memory transfer controller
//   interrupts: Interrupt Controller - Hardware interrupt management and priority handling
//   emu: Emulation Engine - Core timing, context management, and system coordination
//   ui: User Interface - SDL2-based display, input, and debug visualization
//   instructions: Instruction Set - Complete Game Boy instruction decode and execution
//   registers: CPU Registers - Accumulator, flags, and general-purpose register management
//   stack: Stack Operations - Call stack and interrupt stack management
//   io: I/O Registers - Memory-mapped hardware register access
//   debug: Debug Interface - Development tools and state inspection
//   cpu_ops: CPU Operations - Instruction implementation functions
//   cpu_util: CPU Utilities - Helper functions for instruction execution
//   ppu_pipeline: PPU Pipeline - Graphics rendering pipeline stages
//
// System Integration:
//   - All components communicate through the bus module
//   - Timing is synchronized through the emu module's cycle counting
//   - Interrupts coordinate between components via the interrupt controller
//   - Debug information flows through specialized debug modules
//   - UI provides real-time visualization of system state
//
// Accuracy Focus:
//   - Cycle-accurate timing for critical components (CPU, PPU, Timer)
//   - Accurate memory banking behavior for cartridge compatibility
//   - Proper interrupt timing and priority handling
//   - Audio synthesis matching original Game Boy characteristics
//   - LCD timing modes and display behavior emulation

// Core system components
pub mod bus;
pub mod cpu;
pub mod emu;

// Memory and storage
pub mod cart;
pub mod ram;

// Graphics and display
pub mod lcd;
pub mod ppu;
pub mod ppu_pipeline;

// Audio
pub mod apu;

// Input and timing
pub mod dma;
pub mod gamepad;
pub mod timer;

// System infrastructure
pub mod interrupts;
pub mod io;
pub mod stack;

// CPU implementation
pub mod cpu_ops;
pub mod cpu_util;
pub mod instructions;
pub mod registers;

// User interface and debugging
pub mod debug;
pub mod ui;
