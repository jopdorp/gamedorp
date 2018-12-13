//! Game Boy CPU emulation

use io::{Interconnect, Interrupt};
use std::fmt::{Debug, Error, Formatter};

use gb_rs_cpu::instructions::next_instruction;

mod instructions;

/// CPU state.
pub struct Cpu<'a> {
    /// CPU registers (except for `F` register)
    pub regs: Registers,
    /// CPU flags (`F` register)
    pub flags: Flags,
    /// Interrupt enabled flag
    iten: bool,
    /// True if interrupts should be enabled after next instruction
    iten_enable_next: bool,
    /// CPU halted flag
    halted: bool,
    /// Interconnect to access external ressources (RAM, ROM, peripherals...)
    pub inter: Interconnect<'a>,
    /// Number of cycles elapsed running the current instruction
    instruction_cycles: u8,
}

/// CPU registers. They're 16bit wide but some of them can be accessed
/// as high and low byte.
pub struct Registers {
    /// 16bit Program Counter
    pub pc: u16,
    /// 16bit Stack Pointer
    pub sp: u16,
    /// 8bit `A` register
    pub a: u8,
    /// 8bit `B` register
    pub b: u8,
    /// 8bit `C` register
    pub c: u8,
    /// 8bit `D` register
    pub d: u8,
    /// 8bit `E` register
    pub e: u8,
    /// 16bit `HL` register. This register can be split in `H` and `L`
    /// like the others but it's often used as a 16bit memory pointer.
    pub hl: u16,
}

/// Flags contain `bool`s which are set or unset as a side effect of
/// the commands being executed. In turn, certain commands change
/// their behaviour based on the flag values.
pub struct Flags {
    /// Zero: set if the result of a math operation is zero or two
    /// values compare equal
    pub z: bool,
    /// Substract Flag: set if the last math operation performed a
    /// substraction
    pub n: bool,
    /// Half Carry Flag: set if a carry occurred from the lower nibble
    /// in the last math operation.
    pub h: bool,
    /// Carry Flag: set if a carry occured during the last math
    /// operation or if the first operand register compared smaller.
    pub c: bool,
}

impl<'a> Cpu<'a> {
    /// Create a new Cpu instance and reset it
    pub fn new<'n>(inter: Interconnect<'n>) -> Cpu<'n> {
        // Default register values at startup. Taken from the
        // unofficial Game Boy CPU manual.
        let regs = Registers {
            pc: 0,
            sp: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            hl: 0,
        };

        Cpu {
            regs: regs,
            flags: Flags {
                z: false,
                n: false,
                h: false,
                c: false,
            },
            inter: inter,
            iten: true,
            iten_enable_next: true,
            halted: false,
            instruction_cycles: 0,
        }
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
        };

        // Push current value to stack
        let pc = self.pc();
        self.push_word(pc);

        // Context switching delay
        self.delay(6);

        // Jump to IT handler
        self.set_pc(handler_addr);
    }

    /// Fetch byte at `addr` from the interconnect. Takes one machine
    /// cycle.
    fn fetch_byte(&mut self, addr: u16) -> u8 {
        let b = self.inter.fetch_byte(addr);

        self.delay(1);

        b
    }

    /// Store byte `val` at `addr` in the interconnect. Takes one
    /// machine cycle.
    fn store_byte(&mut self, addr: u16, val: u8) {
        self.inter.store_byte(addr, val);

        self.delay(1);
    }

    /// Push one byte onto the stack and decrement the stack pointer
    fn push_byte(&mut self, val: u8) {
        let mut sp = self.sp();

        sp -= 1;

        self.set_sp(sp);
        self.store_byte(sp, val);
    }

    /// Push two bytes onto the stack and decrement the stack pointer
    /// twice
    fn push_word(&mut self, val: u16) {
        self.push_byte((val >> 8) as u8);
        self.push_byte(val as u8);
    }

    /// Retreive one byte from the stack and increment the stack pointer
    fn pop_byte(&mut self) -> u8 {
        let sp = self.sp();

        let b = self.fetch_byte(sp);

        self.set_sp(sp + 1);

        b
    }

    /// Retreive two bytes from the stack and increment the stack pointer
    /// twice
    fn pop_word(&mut self) -> u16 {
        let lo = self.pop_byte() as u16;
        let hi = self.pop_byte() as u16;

        (hi << 8) | lo
    }

    /// Advance the rest of the emulator state. `cycles` is given in
    /// system clock periods.
    fn advance(&mut self, cycles: u8) {
        for _ in 0..cycles {
            self.inter.step();
        }

        self.instruction_cycles += cycles;
    }

    /// Make the CPU wait for the given number of machine
    /// cycles. 1 machine cycle costs 4 sysclk cycles.
    fn delay(&mut self, machine_cycles: u8) {
        self.advance(machine_cycles * 4);
    }

    /// Retrieve value of the `PC` register
    fn pc(&self) -> u16 {
        self.regs.pc
    }

    /// Set value of the `PC` register
    fn set_pc(&mut self, pc: u16) {
        self.regs.pc = pc;
    }

    /// Load value into `PC` register. Takes one machine cycle
    fn load_pc(&mut self, pc: u16) {
        self.set_pc(pc);

        self.delay(1);
    }

    /// Retrieve value of the `SP` register
    fn sp(&self) -> u16 {
        self.regs.sp
    }

    /// Set value of the `SP` register
    fn set_sp(&mut self, sp: u16) {
        self.regs.sp = sp;
    }

    /// Retrieve value of the `AF` register
    fn af(&self) -> u16 {
        let mut v = self.f() as u16;

        v |= (self.regs.a as u16) << 8;

        v
    }

    /// Set value of the `AF` register
    fn set_af(&mut self, af: u16) {
        self.regs.a = (af >> 8) as u8;
        self.set_f(af as u8);
    }

    /// Retrieve value of the `BC` register
    fn bc(&self) -> u16 {
        let mut v = self.regs.c as u16;

        v |= (self.regs.b as u16) << 8;

        v
    }

    /// Set value of the `BC` register
    fn set_bc(&mut self, bc: u16) {
        self.regs.b = (bc >> 8) as u8;
        self.regs.c = bc as u8;
    }

    /// Retrieve value of the `DE` register
    fn de(&self) -> u16 {
        let mut v = self.regs.e as u16;

        v |= (self.regs.d as u16) << 8;

        v
    }

    /// Set value of the `DE` register
    fn set_de(&mut self, de: u16) {
        self.regs.d = (de >> 8) as u8;
        self.regs.e = de as u8;
    }

    /// Retrieve value of the `HL` register
    fn hl(&self) -> u16 {
        self.regs.hl
    }

    /// Set value of the `HL` register
    fn set_hl(&mut self, hl: u16) {
        self.regs.hl = hl
    }

    /// Retrieve value of the `A` register
    fn a(&self) -> u8 {
        self.regs.a
    }

    /// Set value of the `A` register
    fn set_a(&mut self, v: u8) {
        self.regs.a = v;
    }

    /// Retrieve value of the `B` register
    fn b(&self) -> u8 {
        self.regs.b
    }

    /// Set value of the `B` register
    fn set_b(&mut self, v: u8) {
        self.regs.b = v;
    }

    /// Retrieve value of the `C` register
    fn c(&self) -> u8 {
        self.regs.c
    }

    /// Set value of the `C` register
    fn set_c(&mut self, v: u8) {
        self.regs.c = v;
    }

    /// Retrieve value of the `D` register
    fn d(&self) -> u8 {
        self.regs.d
    }

    /// Set value of the `D` register
    fn set_d(&mut self, v: u8) {
        self.regs.d = v;
    }

    /// Retrieve value of the `E` register
    fn e(&self) -> u8 {
        self.regs.e
    }

    /// Set value of the `E` register
    fn set_e(&mut self, v: u8) {
        self.regs.e = v;
    }

    /// Retrieve value of the `F` register
    fn f(&self) -> u8 {
        let z = self.flags.z as u8;
        let n = self.flags.n as u8;
        let h = self.flags.h as u8;
        let c = self.flags.c as u8;

        (z << 7) | (n << 6) | (h << 5) | (c << 4)
    }

    /// Set value of the `F` register
    fn set_f(&mut self, v: u8) {
        self.flags.z = (v & (1 << 7)) != 0;
        self.flags.n = (v & (1 << 6)) != 0;
        self.flags.h = (v & (1 << 5)) != 0;
        self.flags.c = (v & (1 << 4)) != 0;
    }

    /// Retrieve value of the `H` register
    fn h(&self) -> u8 {
        (self.regs.hl >> 8) as u8
    }

    /// Set value of the `H` register
    fn set_h(&mut self, v: u8) {
        let mut hl = self.hl();

        hl &= 0xff;
        hl |= (v as u16) << 8;

        self.set_hl(hl);
    }

    /// Retrieve value of the `L` register
    fn l(&self) -> u8 {
        self.regs.hl as u8
    }

    /// Set value of the `L` register
    fn set_l(&mut self, v: u8) {
        let mut hl = self.hl();

        hl &= 0xff00;
        hl |= v as u16;

        self.set_hl(hl);
    }

    /// Get value of 'Z' flag
    fn zero(&self) -> bool {
        self.flags.z
    }

    /// set value of 'Z' flag
    fn set_zero(&mut self, s: bool) {
        self.flags.z = s;
    }

    /// Get value of 'C' flag
    fn carry(&self) -> bool {
        self.flags.c
    }

    /// Set value of 'C' flag
    fn set_carry(&mut self, s: bool) {
        self.flags.c = s;
    }

    /// Get value of 'H' flag
    fn halfcarry(&self) -> bool {
        self.flags.h
    }

    /// Set value of 'H' flag
    fn set_halfcarry(&mut self, s: bool) {
        self.flags.h = s;
    }

    /// Get value of 'N' flag
    fn substract(&self) -> bool {
        self.flags.n
    }

    /// Set value of 'N' flag
    fn set_substract(&mut self, s: bool) {
        self.flags.n = s;
    }

    /// Disable Interrupts. Takes effect immediately and cancels any
    /// pending interrupt enable request.
    fn disable_interrupts(&mut self) {
        self.iten = false;
        self.iten_enable_next = false;
    }

    /// Enable Interrupts immediately
    fn enable_interrupts(&mut self) {
        self.iten = true;
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

    /// Stop, blank the screen and wait for button press
    fn stop(&mut self) {
        println!("{:?}", *self);
        panic!("STOP is not implemented");
    }

    // Partial reset procedure used for benchmarks
    #[cfg(test)]
    pub fn reset(&mut self) {
        self.set_pc(0);
    }
}

impl<'a> Debug for Cpu<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        try!(writeln!(f, "Registers:"));

        try!(writeln!(
            f,
            "  pc: 0x{:04x} [{:02X} {:02X} {:02X} ...]",
            self.pc(),
            self.inter.fetch_byte(self.pc()),
            self.inter.fetch_byte(self.pc() + 1),
            self.inter.fetch_byte(self.pc() + 2)
        ));
        try!(writeln!(
            f,
            "  sp: 0x{:04x} [{:02X} {:02X} {:02X} ...]",
            self.sp(),
            self.inter.fetch_byte(self.sp()),
            self.inter.fetch_byte(self.sp() + 1),
            self.inter.fetch_byte(self.sp() + 2)
        ));
        try!(writeln!(
            f,
            "  af: 0x{:04x}    a: {:3}    f: {:3}",
            self.af(),
            self.a(),
            self.f()
        ));
        try!(writeln!(
            f,
            "  bc: 0x{:04x}    b: {:3}    c: {:3}",
            self.bc(),
            self.b(),
            self.c()
        ));
        try!(writeln!(
            f,
            "  de: 0x{:04x}    d: {:3}    d: {:3}",
            self.de(),
            self.d(),
            self.e()
        ));
        try!(writeln!(
            f,
            "  hl: 0x{:04x}    h: {:3}    l: {:3}    \
             [hl]: [{:02X} {:02X} ...]",
            self.hl(),
            self.h(),
            self.l(),
            self.inter.fetch_byte(self.hl()),
            self.inter.fetch_byte(self.hl() + 1)
        ));

        try!(writeln!(f, "Flags:"));

        try!(writeln!(
            f,
            "  z: {}  n: {}  h: {}  c: {}",
            self.flags.z as u8, self.flags.n as u8, self.flags.h as u8, self.flags.c as u8
        ));

        try!(writeln!(
            f,
            "  iten: {}  halted: {}",
            self.iten, self.halted
        ));

        Ok(())
    }
}

impl<'n> ::cpu::CanRunInstruction for self::Cpu<'n> {
    /// Execute an instruction or wait for an interrupt if the system
    /// is halted. The rest of the emulator state will be advanced
    /// indirectly by the `advance` method below. The function returns
    /// the number of system clock periods ("ticks") elapsed running
    /// the instruction.
    fn run_next_instruction(&mut self) -> u8 {
        self.instruction_cycles = 0;

        if self.iten {
            if let Some(it) = self.inter.next_interrupt_ack() {
                // We have a pending interrupt!
                self.interrupt(it);
                // Wait until the context switch delay is over. We're
                // sure not to reenter here after that since the
                // `iten` is set to false in `self.interrupt`
                return self.instruction_cycles;
            }
        } else if self.iten_enable_next {
            // If an interrupt enable is pending we update the iten
            // flag
            self.iten = true;
        }

        if self.halted {
            self.inter.step();
            self.instruction_cycles += 1;

            // Check if we have a pending interrupt because even if
            // `iten` is false HALT returns when an IT is triggered
            // (but the IT handler doesn't run)
            if !self.iten && self.inter.next_interrupt().is_some() {
                self.halted = false;
            } else {
                // Wait for interrupt
                return self.instruction_cycles;
            }
        }

        // Now we fetch the next instruction
        let instruction = next_instruction(self);

        // Run the next instruction. This can change the entire CPU
        // state.
        (instruction)(self);

        self.instruction_cycles
    }
}
