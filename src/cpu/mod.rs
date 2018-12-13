extern crate bit_vec;
extern crate time;

use std::thread::sleep;
use std::time::Duration;

use self::bit_vec::BitVec;
use self::time::{precise_time_ns};

use cpu::instructions::INSTRUCTIONS_PIPELINE;
use cpu::instructions::split_into_halves;
use io::{Interconnect, Interrupt};


mod instructions;
mod cpu_test;

pub trait CanRunInstruction {
    fn run_next_instruction(&mut self) -> u8;
}

pub struct Cpu<'a> {
    pub simple_registers: [u8; 6],
    pub accumulator: u8,
    pub flags: [bool; 8], // [0,0,0,0,C,H,N,Z]
    pub stack_pointer: u16,
    pub program_counter: u16,
    pub memory_map:Interconnect<'a>,
    iten: bool,
    iten_enable_next: bool,
    halted: bool,
    instructions_pipeline: Vec<fn(&Cpu,u8) -> bool>,
    last_instruction_codes: Vec<u8>,
    last_pcs: Vec<u16>,
    instruction_cycles: u8
}

impl<'a> Cpu<'a> {
    pub fn new<'n>(inter: Interconnect<'n>) -> Cpu<'n> {
        Cpu {
            simple_registers:  [0; 6], //[B, C, D, E, H, L]
            accumulator: 0,
            flags: [false; 8],
            stack_pointer: 0xFFFE,
            program_counter: 0,
            memory_map: inter,
            iten: true,
            iten_enable_next: true,
            halted: false,
            instructions_pipeline: vec![],
            last_instruction_codes: vec![],
            last_pcs: vec![],
            instruction_cycles: 0
        }
    }



    pub fn read_and_advance_program_counter(&mut self) -> u8 {
        let pc = self.program_counter;
        let instruction_code= self.fetch_byte(pc);
        self.program_counter = self.program_counter.wrapping_add(1);
        return instruction_code;
    }


    pub fn read_and_advance_stack_pointer(&mut self) -> u8 {
        let sp = self.stack_pointer;
        let value= self.fetch_byte(sp);
        self.stack_pointer += 1;
        value
    }

    pub fn push_stack(&mut self, value:u16) {
        trace!("pushing 0x{:x} onto the stack\n", value);
        let sp = self.stack_pointer.wrapping_sub(1);
        self.stack_pointer = sp;
        self.store_byte(sp, ((value & 0xFF00) >> 8) as u8);
        let sp = self.stack_pointer.wrapping_sub(1);
        self.stack_pointer = sp;
        self.store_byte(sp, (value & 0x00FF) as u8);
    }


    pub fn pop_stack(&mut self) -> u16 {
        let sp = self.stack_pointer;
        let lower = self.fetch_byte(sp);
        let sp = self.stack_pointer.wrapping_add(1);
        self.stack_pointer = sp;
        let higher = ((self.fetch_byte(sp) as u16) << 8) as u16;
        let sp = self.stack_pointer.wrapping_add(1);
        self.stack_pointer = sp;
        trace!("popped 0x{:x} from the stack\n",higher | lower as u16);
        higher | lower as u16
    }


    pub fn set_storage_register(&self, index: u8, value: u8) {
        let mut register = self.simple_registers[index as usize];
        register = value;
    }

    pub fn read_hl(&mut self) -> u8 {
        let address = self.read_hl_address();
        self.fetch_byte(address)
    }

    pub fn read_hl_address(&self) -> u16 {
        self.read_combined_register(4)
    }

    pub fn read_de(&mut self) -> u8 {
        let address = self.read_combined_register(2);
        self.fetch_byte(address)
    }

    pub fn read_bc(&mut self) -> u8 {
        let address = self.read_combined_register(0);
        self.fetch_byte(address)
    }

    pub fn read_combined_register(&self, first_register:u8) -> u16{
        if first_register == 6 {
            let higher = ((self.accumulator as u16) << 8);
            let mut lower:u16 = 0;
            for value  in self.flags.iter().rev() {
                lower = (lower << 1) | (*value as u16);
            }
            return higher | lower;
        }
        ((self.simple_registers[first_register as usize] as u16) << 8)
            | self.simple_registers[(first_register + 1) as usize] as u16
    }

    pub fn write_combined_register(&mut self, value:u16, first_register:u8) {
        if first_register == 6 {
            self.accumulator = ((value & 0xFF00) >> 8) as u8;
            let lower = (value & 0x00FF) as u8;
            for (index, value) in BitVec::from_bytes(&[lower]).iter().rev().enumerate() {
                self.flags[index] = value;
            }
            return;
        }
        self.simple_registers[first_register as usize] = ((value & 0xFF00) >> 8) as u8;
        self.simple_registers[(first_register + 1) as usize] = (value & 0x00FF) as u8;
    }

    pub fn read_immediate_value_16(&mut self) -> u16 {
        let value_second_half  = self.read_and_advance_program_counter();
        let value_first_half= self.read_and_advance_program_counter();
        ((value_first_half as u16) << 8) | value_second_half as u16
    }


    /// Disable Interrupts. Takes effect immediately and cancels any
    /// pending interrupt enable request.
    fn disable_interrupts(&mut self) {
        self.iten             = false;
        self.iten_enable_next = false;
    }

    /// Enable Interrupts immediately
    fn enable_interrupts(&mut self) {
        self.iten             = true;
        self.iten_enable_next = true;
    }

    /// Enable Interrupts after the next instruction.
    fn enable_interrupts_next(&mut self) {
        self.iten_enable_next = true;
    }

    /// Halt and wait for interrupts
    fn halt(&mut self) {
        self.halted = true;
    }

    /// Execute interrupt handler for `it`
    fn interrupt(&mut self, it: Interrupt) {

        // If the CPU was halted it's time to wake it up.
        self.halted = false;
        // Interrupt are disabled when entering an interrupt handler.
        self.disable_interrupts();

        let handler_addr = match it {
            Interrupt::VBlank => 0x40,
            Interrupt::Lcdc   => 0x48,
            Interrupt::Timer  => 0x50,
        };

        // Push current value to stack
        let pc = self.program_counter;
        self.push_stack(pc);
        self.delay(6);
        // Jump to IT handler
        self.program_counter = handler_addr;
    }

    pub fn stop(&mut self) {
        panic!("STOP is not implemented");
    }

    fn delay(&mut self, machine_cycles: u8) {
        self.advance(machine_cycles * 4);
    }

    /// Advance the rest of the emulator state. `cycles` is given in
    /// system clock periods.
    fn advance(&mut self, cycles: u8) {
        for _ in 0..cycles {
            self.memory_map.step();
        }

        self.instruction_cycles += cycles;
    }

    /// Fetch byte at `addr` from the interconnect. Takes one machine
       /// cycle.
    fn fetch_byte(&mut self, addr: u16) -> u8 {
        let b = self.memory_map.fetch_byte(addr);

        self.delay(1);

        b
    }

    /// Store byte `val` at `addr` in the interconnect. Takes one
    /// machine cycle.
    fn store_byte(&mut self, addr: u16, val: u8) {
        self.memory_map.store_byte(addr, val);

        self.delay(1);
    }


    /// Load value into `PC` register. Takes one machine cycle
    fn load_pc(&mut self, pc: u16) {
        self.set_pc(pc);

        self.delay(1);
    }

    /// Set value of the `PC` register
    fn set_pc(&mut self, pc: u16) {
        self.program_counter = pc;
    }
}

impl<'n> CanRunInstruction for Cpu<'n> {
    fn run_next_instruction(&mut self) -> u8 {

        self.instruction_cycles = 0;

        if self.iten {
            if let Some(it) = self.memory_map.next_interrupt_ack() {
                // We have a pending interrupt!
                self.interrupt(it);
                // Wait until the context switch delay is over. We're
                // sure not to reenter here after that since the
                // `iten` is set to false in `self.interrupt`
                return self.instruction_cycles;
            }
        } else if self.iten_enable_next {
            self.iten = true;
        }

        if self.halted {
            self.memory_map.step();
            self.instruction_cycles += 1;

            // Check if we have a pending interrupt because even if
            // `iten` is false HALT returns when an IT is triggered
            // (but the IT handler doesn't run)
            if !self.iten && self.memory_map.next_interrupt().is_some() {
                self.halted = false;
            } else {
                // Wait for interrupt
                return self.instruction_cycles;
            }
        }

        trace!("about to read instruction at pc {:x}\n", self.program_counter);

        // for debugging
        let last_pc = self.program_counter.clone();

        let instruction_code= self.read_and_advance_program_counter();
        trace!("about to run instruction {:x}\n", instruction_code);
        let (first_half, second_half) = split_into_halves(instruction_code);
        for instruction in INSTRUCTIONS_PIPELINE.iter() {
            let (found_instruction, _) = instruction(self, instruction_code, first_half, second_half);
            if found_instruction {
                // just for debugging
                if self.last_instruction_codes.len() > 100 {
                    let _ = self.last_instruction_codes.pop();
                    let _ = self.last_pcs.pop();
                }
                let instr: &mut Vec<u8> = &mut vec![instruction_code];
                let pcs: &mut Vec<u16> = &mut vec![last_pc];
                {
                    let old_instr:&mut Vec<u8> = &mut self.last_instruction_codes;
                    instr.append(old_instr);
                    let old_last_pcs:&mut Vec<u16> = &mut self.last_pcs;
                    pcs.append(old_last_pcs);
                }
                self.last_instruction_codes = instr.to_vec();
                self.last_pcs = pcs.to_vec();

                return self.instruction_cycles;
            }
        }

        for (i, value) in self.last_instruction_codes.iter().rev().enumerate() {
            print!( "last instruction and pc 0x{:x}, 0x{:x}\n",
                    value,self.last_pcs[i]);

        }
//        self.delay(1);
        panic!("unsupported opcode 0x{:x} at pc 0x{:x}\n", instruction_code, last_pc);
//        return self.instruction_cycles;
    }
}

