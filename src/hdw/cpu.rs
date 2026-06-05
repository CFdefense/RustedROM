/*
  hdw/cpu.rs
  Info: Central Processing Unit implementation for Game Boy emulation
  Description: The CPU module implements the Sharp LR35902 processor used in the Game Boy.
              Features a complete instruction set with accurate timing, interrupt handling,
              and debug capabilities for development and testing.

  CPU Struct Members:
    registers: CPU Registers - A, B, C, D, E, H, L registers and flags (F) register
    pc: Program Counter - Current instruction address in memory space
    sp: Stack Pointer - Current position in the call stack
    bus: System Bus - Interface to memory, I/O, and other hardware components
    curr_opcode: Current Opcode - The instruction byte currently being executed
    curr_instruction: Current Instruction - Decoded instruction enum for execution
    is_halted: Halt State - CPU halted until interrupt occurs (HALT instruction)
    log_ticks: Debug Logging - Enables detailed execution logging with cycle counts
    debug: Debug Mode - Global debug flag for development features

  CPU Implementation Methods:
    new: Constructor - Initializes CPU with authentic Game Boy register values and debug settings
    step: Execution Cycle - Performs one complete instruction fetch-decode-execute cycle
    fetch: Instruction Fetch - Reads the next opcode from memory at PC address
    decode: Instruction Decode - Converts opcode to executable instruction enum
    execute: Instruction Execute - Matches instruction enum to implementation function
    cpu_request_interrupt: Interrupt Request - Requests hardware interrupt from external components

  Instruction Categories:
    - Arithmetic: ADD, SUB, ADC, SBC, INC, DEC with proper flag handling
    - Logic: AND, OR, XOR, CP with zero and carry flag updates
    - Load/Store: LD variants for registers, memory, and immediate values
    - Control Flow: JP, JR, CALL, RET with conditional and unconditional variants
    - Stack: PUSH, POP for register pairs and return addresses
    - Bit Operations: BIT, SET, RES, SWAP, rotates, and shifts (prefixed CB instructions)
    - System: NOP, HALT, STOP, DI, EI for system control and power management

  Timing and Accuracy:
    - Cycle-accurate execution with proper M-cycle counting
    - Accurate flag behavior for arithmetic and logic operations
    - Proper interrupt timing and master enable/disable handling
    - Authentic register initialization matching real Game Boy boot sequence
    - Correct stack pointer and program counter initialization

  Debug Features:
    - Comprehensive instruction logging with register dumps
    - Cycle count tracking for performance analysis
    - Memory access logging for debugging
    - Interrupt state monitoring
    - CPU state snapshots to files for external analysis

  Integration:
    - Communicates with all system components through the bus interface
    - Coordinates with interrupt controller for hardware interrupt handling
    - Synchronized with PPU for display timing and V-blank interrupts
    - Works with timer for accurate timing interrupt generation
    - Supports DMA operations for high-speed memory transfers
*/

use crate::hdw::bus::BUS;
use crate::hdw::cpu_ops::*;
use crate::hdw::debug_timer::log_timer_state;
use crate::hdw::instructions::*;
use crate::hdw::interrupts::*;
use crate::hdw::registers::*;
use core::panic;

use crate::hdw::emu::EmuContext;
use std::sync::{Arc, Mutex};

use super::cpu_util::{log_cpu_state, print_step_info};
use super::debug;
use super::emu::emu_cycles;

// Our CPU to Call and Control
pub struct CPU {
    pub registers: Registers,
    pub pc: u16,
    pub sp: u16,
    pub bus: BUS,

    pub curr_opcode: u8,
    pub curr_instruction: Option<Instruction>,

    pub is_halted: bool,

    pub log_ticks: bool,
    pub debug: bool,
}
impl CPU {
    // Contructor
    pub fn new(new_bus: BUS, debug: bool) -> Self {
        CPU {
            registers: Registers {
                a: 0x01,
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                f: FlagsRegister {
                    zero: true,
                    subtract: false,
                    half_carry: true,
                    carry: true,
                },
                h: 0x01,
                l: 0x4D,
            },
            pc: 0x0100,
            sp: 0xFFFE,
            bus: new_bus,

            curr_opcode: 0,
            curr_instruction: None,

            is_halted: false,

            log_ticks: debug,
            debug: debug,
        }
    }

    // Function to 'step' through instructions
    pub fn step(&mut self, ctx: Arc<Mutex<EmuContext>>) -> bool {
        if !self.is_halted {
            self.fetch();
            self.decode();

            if self.debug {
                print_step_info(self, &ctx, self.log_ticks);
                log_cpu_state(self, &ctx, self.log_ticks);
                debug::dbg_update(&mut self.bus);
                debug::dbg_print();
            }

            let instruction_to_execute = self.curr_instruction.take();

            if let Some(instruction) = instruction_to_execute {
                log_timer_state(
                    self,
                    &ctx,
                    format!("Executing instruction: {:?}", instruction).as_str(),
                );
                self.execute(instruction); // Execute might modify PC and flags
                if self.log_ticks && self.debug {
                    let ticks = ctx.lock().unwrap().ticks;
                    print!(" {:08X}", ticks);
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("cpu_log.txt")
                    {
                        let _ = std::io::Write::write_all(
                            &mut file,
                            format!(" {:08X}\n", ticks).as_bytes(),
                        );
                    }
                }
            } else {
                panic!("Decode Error: No Instruction")
            }
        } else {
            // is halted
            emu_cycles(self, 1);

            if self.bus.interrupt_controller.get_int_flags() != 0 {
                self.is_halted = false;
                log_timer_state(self, &ctx, "Exiting HALT state due to interrupt");
            }
        }

        // Check for interrupts before executing the next instruction
        if self.bus.interrupt_controller.is_master_enabled() {
            let mut int_controller = std::mem::take(&mut self.bus.interrupt_controller);
            cpu_handle_interrupts(self, &mut int_controller, &ctx);
            self.bus.interrupt_controller = int_controller;
        }

        // Step the interrupt controller to handle delayed IME enabling after EI
        if self.bus.interrupt_controller.step_ime() {
            log_timer_state(self, &ctx, "IME enabled");
        }

        true
    }

    // Function to fetch next opcode
    fn fetch(&mut self) {
        self.curr_opcode = self.bus.read_byte(None, self.pc);
        emu_cycles(self, 1);
    }

    // Function to decode current opcode
    fn decode(&mut self) {
        // Try to decode curr opcode
        self.curr_instruction = Instruction::decode_from_opcode(self.curr_opcode, self.pc, self);

        // Error handling
        if self.curr_instruction.is_none() {
            panic!(
                "Unable to Read Opcode 0x{:02X}, instruction: {:#?}",
                self.curr_opcode, self.curr_instruction
            );
        }
    }

    // Function to execute an opcode by matching Instruction type and target then calling its method
    fn execute(&mut self, instruction: Instruction) {
        match instruction {
            Instruction::NOP => {
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::STOP => {
                println!("STOPPED");
            }
            Instruction::RLCA => {
                op_rlca(self);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::RRCA => {
                op_rrca(self);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::RLA => {
                op_rla(self);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::RRA => {
                op_rra(self);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::DAA => {
                op_daa(self);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::SCF => {
                self.registers.f.carry = true; // C = 1
                self.registers.f.subtract = false; // N = 0
                self.registers.f.half_carry = false; // H = 0
                                                     // Z flag is not affected
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::CPL => {
                op_cpl(self);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::CCF => {
                self.registers.f.carry = !self.registers.f.carry; // C = !C
                self.registers.f.subtract = false; // N = 0
                self.registers.f.half_carry = false; // H = 0
                                                     // Z flag is not affected
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::JR(target) => {
                self.pc = op_jr(self, target);
                self.pc = self.pc.wrapping_add(2); // skip operand of JR
            }
            Instruction::INC(target) => {
                op_inc(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::DEC(target) => {
                op_dec(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::LD(target) => {
                op_ld(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::HALT => {
                self.is_halted = true;
                self.pc = self.pc.wrapping_add(1); // Increment PC after HALT

                // If there's a pending interrupt, exit HALT state immediately
                if (self.bus.interrupt_controller.get_int_flags()
                    & self.bus.interrupt_controller.get_ie_register())
                    != 0
                {
                    self.is_halted = false;
                }
            }
            Instruction::ADD(target) => {
                op_add(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::ADC(target) => {
                op_adc(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::SUB(target) => {
                op_sub(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::SBC(target) => {
                op_sbc(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::AND(target) => {
                let is_d8 = matches!(target, OPTarget::D8);
                op_and(self, target);
                if is_d8 {
                    self.pc = self.pc.wrapping_add(2);
                } else {
                    self.pc = self.pc.wrapping_add(1);
                }
            }
            Instruction::XOR(target) => {
                op_xor(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::OR(target) => {
                op_or(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::CP(target) => {
                op_cp(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::RET(target) => {
                if !op_ret(self, target) {
                    self.pc = self.pc.wrapping_add(1);
                }
            }
            Instruction::RETI => {
                op_reti(self);
            }
            Instruction::POP(target) => {
                op_pop(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::JP(target) => {
                if !op_jp(self, target) {
                    self.pc = self.pc.wrapping_add(3);
                }
            }
            Instruction::CALL(target) => {
                op_call(self, target);
            }
            Instruction::PUSH(target) => {
                op_push(self, target);
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::RST(target) => {
                op_rst(self, target);
            }
            Instruction::DI => {
                self.bus.interrupt_controller.set_master_enabled(false);
                self.bus.interrupt_controller.set_enabling_ime(false); // DI also cancels a pending EI
                self.pc = self.pc.wrapping_add(1);
            }
            Instruction::EI => {
                // EI enables interrupts AFTER the instruction FOLLOWING EI.
                // So, we set a flag to enable IME on the next cycle.
                self.bus.interrupt_controller.set_enabling_ime(true);
                self.pc = self.pc.wrapping_add(1);
            }

            // PREFIXED INSTRUCTIONS: INC PC BY 1 AFTER INSTRUCTION DUE TO CB PREFIX
            Instruction::RLC(target) => {
                op_rlc(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::RRC(target) => {
                op_rrc(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::RL(target) => {
                op_rl(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::RR(target) => {
                op_rr(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::SLA(target) => {
                op_sla(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::SRA(target) => {
                op_sra(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::SWAP(target) => {
                op_swap(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::SRL(target) => {
                op_srl(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::BIT(target) => {
                op_bit(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::RES(target) => {
                op_res(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
            Instruction::SET(target) => {
                op_set(self, target);
                self.pc = self.pc.wrapping_add(2);
            }
        }
    }

    pub fn cpu_request_interrupt(&mut self, interrupt: Interrupts) {
        self.bus.interrupt_controller.request_interrupt(interrupt);
    }
}
