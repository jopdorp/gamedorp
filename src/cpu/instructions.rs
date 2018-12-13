extern crate bit_vec;

use std::collections::HashMap;

use self::bit_vec::BitVec;

pub use cpu::Cpu;

lazy_static! {
    pub static ref LD_IMMEDIATE_VALUE_MAP: HashMap<u8, u8> = vec![
        (0x06_u8, 0),
        (0x0E_u8, 1),
        (0x16_u8, 2),
        (0x1E_u8, 3),
        (0x26_u8, 4),
        (0x2E_u8, 5)
    ]
    .into_iter()
    .collect();
    pub static ref INSTRUCTIONS_PIPELINE: Vec<fn(&mut Cpu, u8, u8, u8) -> (bool, u8)> = vec![
        nop,
        ld_immediate_value_8_bit,
        ld_immediate_value_16_bit,
        load_n_into_hl,
        ld_r1_r2,
        ldd_hl_a,
        ld_n_into_a,
        ld_a_into_n,
        ldh_n_a,
        ldh_a_n,
        ldi_hl_a,
        ldi_a_hl,
        ldd_a_hl,
        ld_sp_hl,
        ldd_a16_sp,
        stop,
        load_a_ff_c,
        load_ff_c_a,
        jump,
        jump_hl,
        jump_a16,
        compare,
        subtract,
        add_a_n,
        add_hl_n,
        xor,
        prefix_cb,
        jump_cc_n,
        jump_cc_nn,
        enable_interrupts,
        disable_interrupts,
        ret,
        ret_cc,
        reti,
        pop,
        push,
        call_immediate_16,
        increment,
        increment_nn,
        decrement_n,
        decrement_nn,
        rla,
        rlca,
        or,
        and,
        daa,
        scf,
        adc,
        cpl,
        rst,
        halt
    ];
    static ref PREFIX_CB_INSTRUCTIONS_PIPELINE: Vec<fn(&mut Cpu, u8, u8, u8) -> (bool, u8)> =
        vec![test_bit, rl_n, swap, res, sla, set, srl];
}

/*************************\
*       INSTRUCTIONS
**************************/
fn nop(_: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0 {
        return (true, 4);
    }
    (false, 0)
}

fn ld_immediate_value_8_bit(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    let register_index = LD_IMMEDIATE_VALUE_MAP.get(&instruction);
    if let Some(register_index) = register_index {
        let value = cpu.read_and_advance_program_counter();
        cpu.simple_registers[*register_index as usize] = value;
        return (true, 8);
    }
    if instruction == 0x36 {
        let value = cpu.read_and_advance_program_counter();
        let address = cpu.read_hl_address();
        cpu.store_byte(address, value);
        return (true, 12);
    }
    (false, 0)
}

fn ld_immediate_value_16_bit(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if second_half == 1 && first_half < 4 {
        let value = cpu.read_immediate_value_16();
        if first_half == 3 {
            cpu.stack_pointer = value;
        } else {
            let offset = first_half * 2;
            cpu.write_combined_register(value, offset);
        }

        return (true, 12);
    }
    (false, 0)
}

fn ldd_hl_a(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x32 {
        let address = cpu.read_hl_address();
        let a = cpu.accumulator;
        cpu.store_byte(address, a);
        cpu.write_combined_register(address.wrapping_sub(1), 4);
        return (true, 8);
    }
    (false, 0)
}

fn ldd_a16_sp(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x8 {
        let address = cpu.read_immediate_value_16();
        let stack_pointer = cpu.stack_pointer;
        cpu.store_byte(address, stack_pointer as u8);
        cpu.store_byte(address.wrapping_add(1), (stack_pointer >> 8) as u8);
        return (true, 20);
    }
    (false, 0)
}

fn ldi_hl_a(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x22 {
        let address = cpu.read_hl_address();
        let a = cpu.accumulator;
        cpu.store_byte(address, a);
        cpu.write_combined_register(address.wrapping_add(1), 4);
        return (true, 8);
    }
    (false, 0)
}

fn ldi_a_hl(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x2A {
        let address = cpu.read_hl_address();
        let value = cpu.read_hl();
        cpu.accumulator = value;
        cpu.write_combined_register(address.wrapping_add(1), 4);
        return (true, 8);
    }
    (false, 0)
}

fn ldd_a_hl(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x3A {
        let address = cpu.read_hl_address();
        let value = cpu.read_hl();
        cpu.accumulator = value;
        cpu.write_combined_register(address.wrapping_sub(1), 4);
        return (true, 8);
    }
    (false, 0)
}

fn ld_n_into_a(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half == 7 && second_half >= 0x8 {
        if second_half == 0xF {
            cpu.accumulator = cpu.accumulator;
        } else if second_half == 0xE {
            cpu.accumulator = cpu.read_hl();
            return (true, 8);
        } else {
            cpu.accumulator = cpu.simple_registers[(second_half - 8) as usize];
        }
        return (true, 4);
    }

    match instruction {
        0x0A => {
            cpu.accumulator = cpu.read_bc();
        }
        0x1A => {
            cpu.accumulator = cpu.read_de();
        }
        0x7E => {
            cpu.accumulator = cpu.read_hl();
        }
        0xFA => {
            let address = cpu.read_immediate_value_16();
            let value = cpu.fetch_byte(address);
            cpu.accumulator = value;
            return (true, 16);
        }
        0x3E => {
            let value = cpu.read_and_advance_program_counter();
            cpu.accumulator = value;
        }
        _ => {
            return (false, 0);
        }
    }
    return (true, 8);
}

fn ld_a_into_n(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    let a = cpu.accumulator;
    match instruction {
        0x7F => {
            cpu.accumulator = a;
        }
        0x47 => {
            cpu.simple_registers[0] = a;
        }
        0x4F => {
            cpu.simple_registers[1] = a;
        }
        0x57 => {
            cpu.simple_registers[2] = a;
        }
        0x5F => {
            cpu.simple_registers[3] = a;
        }
        0x67 => {
            cpu.simple_registers[4] = a;
        }
        0x6F => {
            cpu.simple_registers[5] = a;
        }
        0x02 => {
            let address = ((cpu.simple_registers[0] as u16) << 8) | cpu.simple_registers[1] as u16;
            cpu.store_byte(address, a);
            return (true, 8);
        }
        0x12 => {
            let address = cpu.read_combined_register(2);
            cpu.store_byte(address, a);
            return (true, 8);
        }
        0x77 => {
            let address = ((cpu.simple_registers[4] as u16) << 8) | cpu.simple_registers[5] as u16;
            cpu.store_byte(address, a);
            return (true, 8);
        }
        0xEA => {
            let address = cpu.read_immediate_value_16();
            cpu.store_byte(address, a);
            return (true, 16);
        }
        _ => {
            return (false, 0);
        }
    }
    (true, 4)
}

fn ld_r1_r2(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half >= 4 && first_half <= 6 {
        let offset: u32 = second_half as u32 / 8;
        let first_register_index: u32 = (first_half as u32 - 4) * 2 + offset;
        let second_register_index: u32 = second_half as u32 % 8;

        if first_register_index == 6 {
            load_into_hl(cpu, instruction, second_register_index);
            if instruction == 0x36 {
                return (true, 12);
            }
            return (true, 8);
        } else if second_register_index < 6 {
            let value = cpu.simple_registers[second_register_index as usize];
            if first_register_index < 6 {
                cpu.simple_registers[first_register_index as usize] = value;
            } else if first_register_index == 6 {
                cpu.accumulator = value;
            } else if first_register_index == 7 {
                let address = cpu.read_hl_address();
                cpu.store_byte(address, value);
                return (true, 8);
            }
            return (true, 4);
        } else if second_register_index == 6 {
            let address = cpu.read_hl_address();
            let value = cpu.fetch_byte(address);
            if first_register_index < 6 {
                cpu.simple_registers[first_register_index as usize] = value;
            } else if first_register_index == 6 {
                cpu.accumulator = value;
            } else if first_register_index == 7 {
                let address = cpu.read_hl_address();
                let value = cpu.read_and_advance_stack_pointer();
                cpu.store_byte(address, value);
            }
            return (true, 8);
        }

        return (false, 0);
    }
    (false, 0)
}

fn load_n_into_hl(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half == 0x7 && second_half <= 0x5 {
        let second_register_index: u32 = second_half as u32 % 8;
        load_into_hl(cpu, instruction, second_register_index);
        return (true, 8);
    }
    (false, 0)
}

fn load_a_ff_c(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xF2 {
        let value = cpu
            .memory_map
            .fetch_byte(0xFF00 + cpu.simple_registers[1] as u16);
        cpu.accumulator = value;
        return (true, 8);
    }
    (false, 0)
}

fn load_ff_c_a(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xE2 {
        let address = 0xFF00 | cpu.simple_registers[1] as u16;
        let a = cpu.accumulator;
        cpu.store_byte(address, a);
        return (true, 8);
    }
    (false, 0)
}

fn ldh_n_a(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xE0 {
        let address = 0xFF00 | cpu.read_and_advance_program_counter() as u16;
        let a = cpu.accumulator;
        cpu.store_byte(address, a);
        return (true, 12);
    }
    (false, 0)
}

fn ldh_a_n(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xF0 {
        let address = 0xFF00 | cpu.read_and_advance_program_counter() as u16;
        let value = cpu.fetch_byte(address);
        cpu.accumulator = value;
        return (true, 12);
    }
    (false, 0)
}

fn ld_sp_hl(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xF9 {
        let value = cpu.read_hl_address();
        cpu.stack_pointer = value;
        return (true, 8);
    }
    (false, 0)
}

fn load_into_hl(cpu: &mut Cpu, instruction: u8, second_register_index: u32) {
    let destination_address = cpu.read_hl_address();
    if second_register_index < 6 {
        let value = cpu.simple_registers[second_register_index as usize];
        cpu.store_byte(destination_address, value);
    } else if second_register_index == 7 {
        let value = cpu.read_and_advance_stack_pointer();
        cpu.store_byte(destination_address, value);
    } else if second_register_index == 6 {
        panic!("opcode 76 is halt! read opcode {:x}", instruction);
    }
}

fn compare(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let from = cpu.accumulator.clone();
    let mut to = 0;
    let mut cycles = 0;

    if instruction == 0xBF {
        to = cpu.accumulator;
        cycles = 4;
    } else if instruction == 0xFE {
        to = cpu.read_and_advance_program_counter();
        cycles = 8;
    } else if instruction == 0xBE {
        to = cpu.read_hl();
        cycles = 8;
    } else if first_half == 0xB && second_half <= 0xD && second_half >= 0x8 {
        to = cpu.simple_registers[(second_half - 8) as usize];
        cycles = 4;
    }
    if cycles > 0 {
        subtract_and_set_flags(cpu, from, to);
        return (true, cycles);
    }
    (false, 0)
}

fn subtract(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let a = cpu.accumulator;
    let (b, cycles) = if instruction == 0x97 {
        (cpu.accumulator, 4)
    } else if instruction == 0x96 {
        (cpu.read_hl(), 8)
    } else if instruction == 0xD6 {
        (cpu.read_and_advance_program_counter(), 8)
    } else if first_half == 0x9 && second_half < 0x6 {
        (cpu.simple_registers[second_half as usize], 4)
    } else {
        (0, 0)
    };
    if cycles > 0 {
        cpu.accumulator = subtract_and_set_flags(cpu, a, b);
        return (true, cycles);
    }
    (false, 0)
}

fn add_a_n(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let a = cpu.accumulator;
    let (b, cycles) = if instruction == 0x87 {
        (cpu.accumulator, 4)
    } else if instruction == 0x86 {
        (cpu.read_hl(), 8)
    } else if first_half == 0x8 && second_half < 0x6 {
        (cpu.simple_registers[second_half as usize], 4)
    } else if instruction == 0xC6 {
        (cpu.read_and_advance_program_counter(), 8)
    } else {
        (0, 0)
    };
    if cycles > 0 {
        cpu.accumulator = add_and_set_flags(cpu, a, b);
        return (true, cycles);
    }
    (false, 0)
}

fn add_hl_n(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half <= 3 && second_half == 9 {
        let a = cpu.read_hl_address();
        let first_register = first_half * 2;
        let b = cpu.read_combined_register(first_register);
        let result = add_and_set_flags_16(cpu, a, b);
        cpu.write_combined_register(result, 4);
        return (true, 8);
    }
    (false, 0)
}

fn jump(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xc3 {
        let value = cpu.read_immediate_value_16();
        cpu.load_pc(value);
        return (true, 16);
    }
    (false, 0)
}

fn jump_a16(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xC3 {
        let value = cpu.read_immediate_value_16();
        cpu.load_pc(value);
        return (true, 4);
    }
    (false, 0)
}

fn jump_hl(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xE9 {
        let value = cpu.read_hl_address();
        cpu.set_pc(value);
        return (true, 4);
    }
    (false, 0)
}

fn jump_cc_n(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    let should_jump;
    match instruction {
        0x18 => {
            should_jump = true;
        }
        0x20 => {
            should_jump = !cpu.flags[7];
        }
        0x28 => {
            should_jump = cpu.flags[7];
        }
        0x30 => should_jump = !cpu.flags[4],
        0x38 => should_jump = cpu.flags[4],
        _ => {
            return (false, 0);
        }
    }
    // done here because the immediate value should not be interpreted as an opcode
    let distance = cpu.read_and_advance_program_counter() as i8;
    if should_jump {
        let pc = cpu.program_counter as i16;
        let value = pc.wrapping_add(distance as i16) as u16;
        cpu.load_pc(value);
        return (true, 12);
    }
    (true, 8)
}

fn jump_cc_nn(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    let should_jump;
    match instruction {
        0xC2 => {
            should_jump = !cpu.flags[7];
        }
        0xCA => {
            should_jump = cpu.flags[7];
        }
        0xD2 => should_jump = !cpu.flags[4],
        0xDA => should_jump = cpu.flags[4],
        _ => {
            return (false, 0);
        }
    }
    // done here because the immediate value should not be interpreted as an opcode
    let address = cpu.read_immediate_value_16();
    if should_jump {
        cpu.load_pc(address);
        return (true, 16);
    }
    (true, 12)
}

fn enable_interrupts(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xFB {
        cpu.has_to_enable_interrupts_next = true;
        return (true, 4);
    }
    (false, 0)
}

fn disable_interrupts(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xF3 {
        cpu.disable_interrupts();
        return (true, 4);
    }
    (false, 0)
}

fn call_immediate_16(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xCD {
        let jump_to = cpu.read_immediate_value_16();
        let address = cpu.program_counter;
        cpu.push_stack(address);
        cpu.load_pc(jump_to);
        return (true, 24);
    }
    (false, 0)
}

fn ret(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xC9 {
        do_return(cpu);
        return (true, 16);
    }
    (false, 0)
}

fn ret_cc(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xC0 {
        if !cpu.flags[7] {
            do_return(cpu);
        }
        cpu.delay(1);
        return (true, 8);
    }
    if instruction == 0xC8 {
        if cpu.flags[7] {
            do_return(cpu);
        }
        cpu.delay(1);
        return (true, 8);
    }
    if instruction == 0xD0 {
        if !cpu.flags[4] {
            do_return(cpu);
        }
        cpu.delay(1);
        return (true, 8);
    }
    if instruction == 0xD8 {
        if cpu.flags[4] {
            do_return(cpu);
        }
        cpu.delay(1);
        return (true, 8);
    }
    (false, 0)
}

fn reti(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xD9 {
        do_return(cpu);
        cpu.enable_interrupts();
        return (true, 16);
    }
    (false, 0)
}

pub fn pop(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if second_half == 1 && first_half >= 0xC {
        let value = cpu.pop_stack();
        cpu.write_combined_register(value, (first_half - 0xC) * 2);
        return (true, 12);
    }
    (false, 0)
}

pub fn push(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if second_half == 5 && first_half >= 0xC {
        let value = cpu.read_combined_register((first_half - 0xC) * 2);
        cpu.push_stack(value);
        cpu.delay(1);
        return (true, 16);
    }
    (false, 0)
}

fn increment(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half <= 3 && (second_half == 0xC || second_half == 0x4) {
        match instruction {
            0x3C => {
                let value = cpu.accumulator.clone();
                set_flags_for_increment(cpu, value);
                cpu.accumulator = value.wrapping_add(1);
                return (true, 4);
            }
            0x34 => {
                let mut value = cpu.read_hl();
                set_flags_for_increment(cpu, value);
                value = value.wrapping_add(1);
                let address = cpu.read_hl_address();
                cpu.store_byte(address, value);
                return (true, 12);
            }
            _ => {
                let register = (first_half * 2) + second_half / 8;
                let value = cpu.simple_registers[register as usize];
                set_flags_for_increment(cpu, value);
                let new_value = value.wrapping_add(1);
                cpu.simple_registers[register as usize] = new_value;
                return (true, 4);
            }
        }
    }
    (false, 0)
}

fn increment_nn(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if second_half == 3 && first_half <= 3 {
        let first_register = first_half * 2;
        let value = cpu.read_combined_register(first_register).wrapping_add(1);
        cpu.write_combined_register(value, first_register);
        cpu.delay(1);
        return (true, 8);
    }
    (false, 0)
}

fn decrement_n(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half <= 3 && (second_half == 5 || second_half == 0xD) {
        if instruction == 0x3D {
            let value = cpu.accumulator;
            set_flags_for_decrement(cpu, value);
            cpu.accumulator = value.wrapping_sub(1);
        } else if instruction == 0x35 {
            let value = cpu.read_hl();
            set_flags_for_decrement(cpu, value);
            let address = cpu.read_hl_address();
            cpu.store_byte(address, value.wrapping_sub(1));
            return (true, 12);
        } else {
            let register = first_half * 2 + ((second_half - 5) / 8);
            let value = cpu.simple_registers[register as usize];
            set_flags_for_decrement(cpu, value);
            cpu.simple_registers[register as usize] = value.wrapping_sub(1);
        }
        return (true, 4);
    }
    (false, 0)
}

fn decrement_nn(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half <= 3 && second_half == 0xB {
        let first_register = first_half * 2;
        let value = cpu.read_combined_register(first_register);
        cpu.write_combined_register(value.wrapping_sub(1), first_register);
        cpu.delay(1);
        return (true, 8);
    } else if instruction == 0x3B {
        let value = cpu.stack_pointer;
        cpu.stack_pointer = value.wrapping_sub(1);
        cpu.delay(1);
        return (true, 8);
    }
    (false, 0)
}

fn rla(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x17 {
        let new_value = rotate_left(cpu.accumulator, cpu);
        cpu.accumulator = new_value;
        return (true, 4);
    }

    (false, 0)
}

fn rlca(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x7 {
        let a = cpu.accumulator;
        let c = a >> 7;

        cpu.accumulator = (a << 1) | c;

        cpu.flags[4] = c != 0;
        cpu.flags[5] = false;
        cpu.flags[6] = false;
        cpu.flags[7] = false;
        return (true, 4);
    }

    (false, 0)
}

fn or(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let mut cycles = 0;
    let mut b = 0;
    if instruction == 0xB7 {
        b = cpu.accumulator;
        cycles = 4;
    } else if instruction == 0xB6 {
        b = cpu.read_hl();
        cycles = 8;
    } else if instruction == 0xF6 {
        b = cpu.read_and_advance_program_counter();
        cycles = 8;
    } else if first_half == 0xB && second_half <= 5 {
        b = cpu.simple_registers[second_half as usize];
        cycles = 4;
    }

    if cycles > 0 {
        let a = cpu.accumulator;
        cpu.flags[4] = false;
        cpu.flags[5] = false;
        cpu.flags[6] = false;
        let result = a | b;
        cpu.flags[7] = result == 0;
        cpu.accumulator = result;
        return (true, cycles);
    }
    (false, 0)
}

fn and(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let mut cycles = 0;
    let mut b = 0;
    if instruction == 0xA7 {
        b = cpu.accumulator;
        cycles = 4;
    } else if instruction == 0xA6 {
        b = cpu.read_hl();
        cycles = 8;
    } else if instruction == 0xE6 {
        b = cpu.read_and_advance_program_counter();
        cycles = 8;
    } else if first_half == 0xA && second_half <= 5 {
        b = cpu.simple_registers[second_half as usize];
        cycles = 4;
    }

    if cycles > 0 {
        let a = cpu.accumulator;
        cpu.flags[4] = false;
        cpu.flags[5] = true;
        cpu.flags[6] = false;
        let result = a & b;
        cpu.flags[7] = result == 0;
        cpu.accumulator = result;
        return (true, cycles);
    }
    (false, 0)
}

fn xor(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half == 0xA {
        if second_half >= 0x8 && second_half <= 0xD {
            let value = cpu.simple_registers[(second_half - 8) as usize];
            cpu.accumulator = cpu.accumulator ^ value;
            set_flags_for_xor(cpu);
            return (true, 4);
        }
        if second_half == 0xE {
            cpu.accumulator = cpu.accumulator ^ cpu.read_hl();
            set_flags_for_xor(cpu);
            return (true, 8);
        }

        if second_half == 0xF {
            cpu.accumulator = cpu.accumulator ^ cpu.accumulator;
            set_flags_for_xor(cpu);
            return (true, 4);
        }
    }

    if instruction == 0xEE {
        let value = cpu.read_and_advance_program_counter();
        cpu.accumulator = cpu.accumulator ^ value;
        set_flags_for_xor(cpu);
        return (true, 8);
    }

    (false, 0)
}

fn daa(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x27 {
        let a = cpu.accumulator;
        let mut adjust = 0;

        // See if we had a carry/borrow for the low nibble in the last
        // operation
        if cpu.flags[5] {
            // Yes, we have to adjust it.
            adjust |= 0x06;
        }

        // See if we had a carry/borrow for the high nibble in the last
        // operation
        if cpu.flags[4] {
            // Yes, we have to adjust it.
            adjust |= 0x60;
        }

        let res = if cpu.flags[6] {
            // If the operation was a substraction we're done since we
            // can never end up in the A-F range by substracting
            // without generating a (half)carry.
            a.wrapping_sub(adjust)
        } else {
            // Additions are a bit more tricky because we might have
            // to adjust even if we haven't overflowed (and no carry
            // is present). For instance: 0x8 + 0x4 -> 0xc.
            if a & 0x0F > 0x09 {
                adjust |= 0x06;
            }

            if a > 0x99 {
                adjust |= 0x60;
            }

            a.wrapping_add(adjust)
        };

        cpu.accumulator = res;

        cpu.flags[7] = res == 0;
        cpu.flags[4] = adjust & 0x60 != 0;
        cpu.flags[5] = false;
        return (true, 4);
    }
    (false, 0)
}

fn scf(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x37 {
        cpu.flags[4] = true;
        cpu.flags[5] = false;
        cpu.flags[6] = false;
        return (true, 4);
    }
    (false, 0)
}

/// Helper function to add two `u8`s with carry and update the CPU flags
fn adc(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let mut cycles = 0;
    let mut b = 0;
    if first_half == 0x8 && second_half >= 0x8 && second_half <= 0xD {
        b = cpu.simple_registers[(second_half - 8) as usize];
        cycles = 4;
    } else if instruction == 0x8F {
        b = cpu.accumulator;
        cycles = 4;
    } else if instruction == 0x8E {
        b = cpu.read_hl();
        cycles = 8;
    } else if instruction == 0xCE {
        b = cpu.read_and_advance_program_counter();
        cycles = 8;
    }
    if cycles != 0 {
        let a = cpu.accumulator;

        let result = a.wrapping_add(b).wrapping_add(cpu.flags[4] as u8);

        let result_byte = result as u8;

        cpu.flags[7] = result_byte == 0;
        cpu.flags[5] = (a ^ b ^ result) & 0x10 != 0;
        cpu.flags[4] = result as u16 & 0x100 != 0;
        cpu.flags[6] = false;

        cpu.accumulator = result_byte;
        return (true, cycles);
    }
    (false, 0)
}

fn stop(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x10 {
        let _ = cpu.read_and_advance_program_counter();
        cpu.stop();
        return (true, 4);
    }
    (false, 0)
}

fn cpl(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x2F {
        let value = cpu.accumulator;
        cpu.accumulator = !value;
        cpu.flags[5] = true;
        cpu.flags[6] = true;
        return (true, 4);
    }
    (false, 0)
}

fn rst(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half >= 0xC && (second_half == 0x7 || second_half == 0xF) {
        let address = ((first_half - 0xC) << 4) | (second_half - 7);
        let program_counter = cpu.program_counter;
        cpu.push_stack(program_counter);
        cpu.load_pc(address as u16);
        return (true, 16);
    }
    (false, 0)
}

fn halt(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0x76 {
        cpu.halted = true;
        return (true, 4);
    }
    (false, 0)
}

/*************************\
*        PREFIX_CB
**************************/
fn prefix_cb(cpu: &mut Cpu, instruction: u8, _: u8, _: u8) -> (bool, u8) {
    if instruction == 0xCB {
        let new_instruction_code = cpu.read_and_advance_program_counter();
        trace!("about to run cb opcode {:x}\n", new_instruction_code);
        let (first_half, second_half) = split_into_halves(new_instruction_code);
        for instruction in PREFIX_CB_INSTRUCTIONS_PIPELINE.iter() {
            let (found_instruction, cycles) =
                instruction(cpu, new_instruction_code, first_half, second_half);
            if found_instruction {
                return (true, cycles);
            }
        }
        panic!("unsupported prefix cb opcode {:x}\n", new_instruction_code);
    }
    (false, 0)
}

fn test_bit(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half >= 4 && first_half <= 7 {
        let bit_index: u8 = (first_half - 4) * 2 + second_half / 8;
        let register_index = second_half % 8;
        let bit_is_zero;
        let cycles;
        if register_index == 6 {
            bit_is_zero = is_bit_zero(cpu.read_hl(), bit_index);
            cycles = 16;
        } else if register_index == 7 {
            bit_is_zero = is_bit_zero(cpu.accumulator, bit_index);
            cycles = 8;
        } else {
            bit_is_zero = is_bit_zero(cpu.simple_registers[register_index as usize], bit_index);
            cycles = 8;
        }
        if cycles != 0 {
            let c = cpu.flags[4];
            cpu.flags = [false, false, false, false, c, true, false, bit_is_zero];
            return (true, cycles);
        }
    }
    (false, 0)
}

fn rl_n(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half == 1 && second_half <= 7 {
        if second_half == 6 {
            let new_value = rotate_left(cpu.read_hl(), cpu);
            let address = cpu.read_hl_address();
            cpu.store_byte(address, new_value);
            return (true, 16);
        } else if second_half == 7 {
            cpu.accumulator = rotate_left(cpu.accumulator, cpu);
        } else {
            let new_value = rotate_left(cpu.simple_registers[first_half as usize], cpu);
            cpu.simple_registers[first_half as usize] = new_value;
        }
        return (true, 8);
    }
    (false, 0)
}

fn swap(cpu: &mut Cpu, instruction: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    let value;
    if first_half == 3 && second_half <= 5 {
        value = cpu.simple_registers[first_half as usize];
        let swapped = swap_nibles(value);
        cpu.simple_registers[first_half as usize] = swapped;
        set_flags_for_swap(cpu, swapped);
        return (true, 8);
    } else if instruction == 0x37 {
        value = cpu.accumulator;
        let swapped = swap_nibles(value);
        cpu.accumulator = swapped;
        set_flags_for_swap(cpu, swapped);
        return (true, 8);
    } else if instruction == 0x36 {
        value = cpu.read_hl();
        let swapped = swap_nibles(value);
        let address = cpu.read_hl_address();
        cpu.store_byte(address, swapped);
        set_flags_for_swap(cpu, swapped);
        return (true, 16);
    }
    (false, 0)
}

fn res(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half >= 0x8 && first_half <= 0xB {
        let bit = (first_half - 8) * 2 + second_half / 8;
        let register_index = second_half % 8;

        if register_index <= 5 {
            let value = cpu.simple_registers[register_index as usize];
            cpu.simple_registers[register_index as usize] = calculate_res(value, bit);
            return (true, 8);
        } else if register_index == 6 {
            let value = cpu.read_hl();
            let address = cpu.read_hl_address();
            cpu.store_byte(address, calculate_res(value, bit));
            return (true, 16);
        } else if register_index == 7 {
            let value = cpu.accumulator;
            cpu.accumulator = calculate_res(value, bit);
            return (true, 8);
        }
    }
    (false, 0)
}

fn sla(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half == 0x2 && second_half <= 7 {
        if second_half == 6 {
            let value = cpu.read_hl();
            let result = calculate_sla_and_set_flags(cpu, value);
            let address = cpu.read_hl_address();
            cpu.store_byte(address, result);
            return (true, 16);
        } else if second_half == 7 {
            let value = cpu.accumulator;
            let result = calculate_sla_and_set_flags(cpu, value);
            cpu.accumulator = result;
            return (true, 8);
        } else {
            let value = cpu.simple_registers[second_half as usize];
            let result = calculate_sla_and_set_flags(cpu, value);
            cpu.simple_registers[second_half as usize] = result;
            return (true, 8);
        }
    }
    (false, 0)
}

/// Shift `A` to the left
fn calculate_sla_and_set_flags(cpu: &mut self::Cpu, v: u8) -> u8 {
    cpu.flags[4] = v & 0x80 != 0;

    let r = v << 1;

    cpu.flags[7] = r == 0;

    cpu.flags[6] = false;
    cpu.flags[5] = false;

    r
}

fn set(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half >= 0xC {
        let bit = (first_half - 0xC) * 2 + second_half / 8;
        let register_index = second_half % 8;

        if register_index <= 5 {
            let value = cpu.simple_registers[register_index as usize];
            cpu.simple_registers[register_index as usize] = calculate_set(value, bit);
            return (true, 8);
        } else if register_index == 6 {
            let value = cpu.read_hl();
            let address = cpu.read_hl_address();
            cpu.store_byte(address, calculate_set(value, bit));
            return (true, 16);
        } else if register_index == 7 {
            let value = cpu.accumulator;
            cpu.accumulator = calculate_set(value, bit);
            return (true, 8);
        }
    }
    (false, 0)
}

fn calculate_set(val: u8, bit: u8) -> u8 {
    val | (1u8 << (bit as usize))
}

fn srl(cpu: &mut Cpu, _: u8, first_half: u8, second_half: u8) -> (bool, u8) {
    if first_half == 0x3 && second_half >= 8 {
        if second_half == 0xE {
            let value = cpu.read_hl();
            let result = calculate_srl(cpu, value);
            let address = cpu.read_hl_address();
            cpu.store_byte(address, result);
            return (true, 16);
        } else if second_half == 0xF {
            let value = cpu.accumulator;
            let result = calculate_srl(cpu, value);
            cpu.accumulator = result;
            return (true, 8);
        } else {
            let value = cpu.simple_registers[(second_half + 8) as usize];
            let result = calculate_srl(cpu, value);
            cpu.simple_registers[(second_half + 8) as usize] = result;
            return (true, 8);
        }
    }
    (false, 0)
}

fn calculate_srl(cpu: &mut self::Cpu, v: u8) -> u8 {
    cpu.flags[4] = v & 1 != 0;

    let r = v >> 1;

    cpu.flags[7] = r == 0;
    cpu.flags[6] = false;
    cpu.flags[5] = false;

    r
}

/*************************\
*         HELPERS
**************************/
fn calculate_res(value: u8, bit: u8) -> u8 {
    value & !(1u8 << (bit as usize))
}
fn rotate_left(value: u8, cpu: &mut Cpu) -> u8 {
    let new_value = (value << 1) | cpu.flags[4] as u8;
    cpu.flags[4] = (value >> 7) != 0;
    cpu.flags[5] = false;
    cpu.flags[6] = false;
    cpu.flags[7] = new_value == 0;
    new_value
}

fn is_bit_zero(value: u8, bit_index: u8) -> bool {
    false == BitVec::from_bytes(&[value])[7 - bit_index as usize]
}

fn set_flags_for_swap(cpu: &mut Cpu, value: u8) {
    cpu.flags[4] = false;
    cpu.flags[5] = false;
    cpu.flags[6] = false;
    cpu.flags[7] = value == 0;
}

fn set_flags_for_decrement(cpu: &mut Cpu, value: u8) {
    cpu.flags[5] = value & 0xf == 0;
    cpu.flags[6] = true;
    cpu.flags[7] = value.wrapping_sub(1) == 0;
}

fn set_flags_for_increment(cpu: &mut Cpu, value: u8) {
    let current_flags = cpu.flags.to_owned();
    cpu.flags = [
        false,
        false,
        false,
        false,
        current_flags[4],
        value & 0xf == 0xf,
        false,
        value.wrapping_add(1) == 0,
    ];
}

fn do_return(cpu: &mut Cpu) {
    let address = cpu.pop_stack();
    cpu.load_pc(address);
}

fn subtract_and_set_flags(cpu: &mut Cpu, a: u8, b: u8) -> u8 {
    // Check for borrow using 32bit arithmetics
    let a = a as u32;
    let b = b as u32;

    let result = a.wrapping_sub(b);

    let result_byte = result as u8;

    cpu.flags = [
        false,
        false,
        false,
        false,
        result & 0x100 != 0,
        (a ^ b ^ result) & 0x10 != 0,
        true,
        result_byte == 0,
    ];
    result_byte
}

fn add_and_set_flags(cpu: &mut Cpu, a: u8, b: u8) -> u8 {
    // Check for borrow using 32bit arithmetics
    let a = a as u32;
    let b = b as u32;

    let result = a.wrapping_add(b);

    let result_byte = result as u8;

    cpu.flags = [
        false,
        false,
        false,
        false,
        result & 0x100 != 0,
        (a ^ b ^ result) & 0x10 != 0,
        false,
        result_byte == 0,
    ];
    result_byte
}

fn add_and_set_flags_16(cpu: &mut Cpu, a: u16, b: u16) -> u16 {
    let a = a as u32;
    let b = b as u32;

    let result = a.wrapping_add(b);

    cpu.flags[4] = result & 0x10000 != 0;
    cpu.flags[5] = (a ^ b ^ result) & 0x1000 != 0;
    cpu.flags[6] = false;
    cpu.delay(1);

    result as u16
}

fn swap_nibles(byte: u8) -> u8 {
    let (first, second) = split_into_halves(byte);
    return second << 4 | first;
}

pub fn split_into_halves(byte: u8) -> (u8, u8) {
    (get_first_half(byte), get_second_half(byte))
}

fn get_first_half(byte: u8) -> u8 {
    let mut fh = BitVec::from_bytes(&[byte]);
    fh.intersect(&BitVec::from_bytes(&[0b11110000]));
    fh.to_bytes()[0] >> 4
}

fn get_second_half(byte: u8) -> u8 {
    let mut sh = BitVec::from_bytes(&[byte]);
    sh.intersect(&BitVec::from_bytes(&[0b00001111]));
    let second_half: u8 = sh.to_bytes()[0];
    second_half
}

fn set_flags_for_xor(cpu: &mut Cpu) {
    cpu.flags = [
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        cpu.accumulator == 0,
    ];
}
