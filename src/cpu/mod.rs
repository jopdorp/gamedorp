extern crate bit_vec;

use self::bit_vec::BitVec;

use cpu::instructions::split_into_halves;
use cpu::instructions::INSTRUCTIONS_PIPELINE;
use io::{Interconnect, Interrupt};

mod cpu_test;
mod instructions;

pub trait CanRunInstruction {
    fn run_next_instruction(&mut self) -> u8;
}

pub struct Cpu<'a> {
    pub simple_registers: [u8; 6],
    pub accumulator: u8,
    pub flags: [bool; 8], // [0,0,0,0,C,H,N,Z]
    pub stack_pointer: u16,
    pub program_counter: u16,
    pub memory_map: Interconnect<'a>,
    is_interrupts_enabled: bool,
    has_to_enable_interrupts_next: bool,
    halted: bool,
    instruction_cycles: u8,
}

impl<'a> Cpu<'a> {
    pub fn new<'n>(inter: Interconnect<'n>) -> Cpu<'n> {
        Cpu {
            simple_registers: [0; 6], //[B, C, D, E, H, L]
            accumulator: 0,
            flags: [false; 8],
            stack_pointer: 0xFFFE,
            program_counter: 0,
            memory_map: inter,
            is_interrupts_enabled: true,
            has_to_enable_interrupts_next: true,
            halted: false,
            instruction_cycles: 0,
        }
    }

    pub fn read_and_advance_program_counter(&mut self) -> u8 {
        let pc = self.program_counter;
        let instruction_code = self.fetch_byte(pc);
        self.program_counter = self.program_counter.wrapping_add(1);
        return instruction_code;
    }

    pub fn read_and_advance_stack_pointer(&mut self) -> u8 {
        let sp = self.stack_pointer;
        let value = self.fetch_byte(sp);
        self.stack_pointer += 1;
        value
    }

    pub fn push_stack(&mut self, value: u16) {
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
        trace!("popped 0x{:x} from the stack\n", higher | lower as u16);
        higher | lower as u16
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

    pub fn read_combined_register(&self, first_register: u8) -> u16 {
        if first_register == 6 {
            let higher = (self.accumulator as u16) << 8;
            let mut lower: u16 = 0;
            for value in self.flags.iter().rev() {
                lower = (lower << 1) | (*value as u16);
            }
            return higher | lower;
        }
        ((self.simple_registers[first_register as usize] as u16) << 8)
            | self.simple_registers[(first_register + 1) as usize] as u16
    }

    pub fn write_combined_register(&mut self, value: u16, first_register: u8) {
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
        let value_second_half = self.read_and_advance_program_counter();
        let value_first_half = self.read_and_advance_program_counter();
        ((value_first_half as u16) << 8) | value_second_half as u16
    }

    /// Disable Interrupts. Takes effect immediately and cancels any
    /// pending interrupt enable request.
    fn disable_interrupts(&mut self) {
        self.is_interrupts_enabled = false;
        self.has_to_enable_interrupts_next = false;
    }

    /// Enable Interrupts immediately
    fn enable_interrupts(&mut self) {
        self.is_interrupts_enabled = true;
        self.has_to_enable_interrupts_next = true;
    }

    /// Execute interrupt handler for `it`
    fn interrupt(&mut self, it: Interrupt) {
        // If the CPU was halted it's time to wake it up.
        self.halted = false;
        // Interrupt are disabled when entering an interrupt handler.
        self.disable_interrupts();

        let handler_addr = match it {
            Interrupt::VBlank => 0x40,
            Interrupt::Lcdc => 0x48,
            Interrupt::Timer => 0x50,
            Interrupt::Button => 0x60,
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

    fn advance(&mut self, cycles: u8) {
        for _ in 0..cycles {
            self.memory_map.step();
        }

        self.instruction_cycles += cycles;
    }

    fn fetch_byte(&mut self, addr: u16) -> u8 {
        let b = self.memory_map.fetch_byte(addr);

        self.delay(1);

        b
    }

    fn store_byte(&mut self, addr: u16, val: u8) {
        self.memory_map.store_byte(addr, val);
        self.delay(1);
    }

    fn load_pc(&mut self, pc: u16) {
        self.set_pc(pc);
        self.delay(1);
    }

    fn set_pc(&mut self, pc: u16) {
        self.program_counter = pc;
    }
}

impl<'n> CanRunInstruction for Cpu<'n> {
    fn run_next_instruction(&mut self) -> u8 {
        self.instruction_cycles = 0;

        if self.is_interrupts_enabled {
            if let Some(it) = self.memory_map.next_interrupt_ack() {
                // We have a pending interrupt!
                self.interrupt(it);
                // Wait until the context switch delay is over. We're
                // sure not to reenter here after that since the
                // `iten` is set to false in `self.interrupt`
                return self.instruction_cycles;
            }
        } else if self.has_to_enable_interrupts_next {
            self.is_interrupts_enabled = true;
        }

        if self.halted {
            self.memory_map.step();
            self.instruction_cycles += 1;

            // Check if we have a pending interrupt because even if
            // `iten` is false HALT returns when an IT is triggered
            // (but the IT handler doesn't run)
            if !self.is_interrupts_enabled && self.memory_map.next_interrupt().is_some() {
                self.halted = false;
            } else {
                // Wait for interrupt
                return self.instruction_cycles;
            }
        }

        trace!(
            "about to read instruction at pc {:x}\n",
            self.program_counter
        );
        let instruction_code = self.read_and_advance_program_counter();
        trace!("about to run instruction {:x}\n", instruction_code);
        let (first_half, second_half) = split_into_halves(instruction_code);
        for instruction in INSTRUCTIONS_PIPELINE.iter() {
            if instruction(self, instruction_code, first_half, second_half) {
                return self.instruction_cycles;
            }
        }
        panic!(
            "unsupported opcode 0x{:x} at pc 0x{:x}\n",
            instruction_code, self.program_counter
        );
    }
}
