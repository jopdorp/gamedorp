extern crate bit_vec;
extern crate typenum;

use std::collections::HashMap;
use std::mem::transmute;
use std::num::Wrapping;

use self::bit_vec::BitVec;
use self::typenum::U1;

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
    pub static ref INSTRUCTIONS_PIPELINE: Vec<fn(&mut Cpu, u8) -> (bool, u8)> = vec![
        nop,
        ld_immediate_value_8_bit,
        ld_immediate_value_16_bit,
        ld_r1_r2,
        ldd_hl_a,
        ld_n_into_a,
        ld_a_into_n,
        ldh_n_a,
        ldh_a_n,
        ldi_hl_a,
        ld_sp_hl,
        load_a_ff_c,
        load_ff_c_a,
        jump,
        compare,
        subtract,
        xor,
        prefix_cb,
        jump_cc_n,
        enable_interrupts,
        disable_interrupts,
        ret,
        ret_cc,
        pop,
        push,
        call_immediate_16,
        increment,
        increment_nn,
        decrement_n,
        rla
    ];
    static ref PREFIX_CB_INSTRUCTIONS_PIPELINE: Vec<fn(&mut Cpu, u8) -> (bool, u8)> = vec![
        test_bit,
        rl_n
    ];
}

/*************************\
*       INSTRUCTIONS
**************************/
fn nop(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0 {
        return (true, 4);
    }
    (false, 0)
}

fn ld_immediate_value_8_bit(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let register_index = LD_IMMEDIATE_VALUE_MAP.get(&instruction);
    if let Some(register_index) = register_index {
        let value = cpu.read_and_advance_program_counter();
        cpu.simple_registers[*register_index as usize] = value;
        return (true, 8);
    }
    (false, 0)
}

fn ld_immediate_value_16_bit(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half) = split_into_halves(instruction);
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

fn ldd_hl_a(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0x32 {
        let address = cpu.read_hl_address();
        cpu.memory_map.store_byte(address, cpu.accumulator);
        cpu.write_combined_register(address.wrapping_sub(1),4);
        return (true, 8);
    }
    (false, 0)
}

fn ldi_hl_a(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0x22 {
        let mut address = cpu.read_hl_address();
        cpu.memory_map.store_byte(address.clone(), cpu.accumulator);
        cpu.write_combined_register(wrapping_increment_16(address),4);
        return (true, 8);
    }
    (false, 0)
}

fn ld_n_into_a(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let first_half: u32 = get_first_half(instruction) as u32;
    let second_half = get_second_half(instruction);
    if first_half == 7 && second_half >= 0x8 {
        if second_half == 0xF {
            cpu.accumulator = cpu.accumulator;
        } else if second_half == 0xE {
            cpu.accumulator = cpu.read_hl();
        } else {
            cpu.accumulator = cpu.simple_registers[(second_half - 8) as usize];
        }
        return (true, 8);
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
            let value = cpu.memory_map.fetch_byte(address);
            cpu.accumulator = value;
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

fn ld_a_into_n(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    match instruction {
        0x7F => {
            cpu.accumulator = cpu.accumulator;
        }
        0x47 => {
            cpu.simple_registers[0] = cpu.accumulator;
        }
        0x4F => {
            cpu.simple_registers[1] = cpu.accumulator;
        }
        0x57 => {
            cpu.simple_registers[2] = cpu.accumulator;
        }
        0x5F => {
            cpu.simple_registers[3] = cpu.accumulator;
        }
        0x67 => {
            cpu.simple_registers[4] = cpu.accumulator;
        }
        0x6F => {
            cpu.simple_registers[5] = cpu.accumulator;
        }
        0x02 => {
            let address =
                ((cpu.simple_registers[0] as u16) << 8) | cpu.simple_registers[1] as u16;
            cpu.memory_map.store_byte(address, cpu.accumulator);
            return (true, 8);
        }
        0x12 => {
            let address = cpu.read_combined_register(2);
            cpu.memory_map.store_byte(address, cpu.accumulator);
            return (true, 8);
        }
        0x77 => {
            let address =
                ((cpu.simple_registers[4] as u16) << 8) | cpu.simple_registers[5] as u16;
            cpu.memory_map.store_byte(address, cpu.accumulator);
            return (true, 8);
        }
        0xEA => {
            let address = cpu.read_immediate_value_16();
            let value = cpu.accumulator;
            cpu.memory_map.store_byte(address, value);
            return (true, 16);
        }
        _ => {
            return (false, 0);
        }
    }
    (true, 8)
}

fn ld_r1_r2(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let first_half: u32 = get_first_half(instruction) as u32;
    let second_half = get_second_half(instruction);

    if first_half >= 4 && first_half <= 6 {
        let offset: u32 = second_half as u32 / 8;
        let first_register_index: u32 = (first_half - 4) * 2 + offset;
        let second_register_index: u32 = second_half as u32 % 8;

        if first_register_index == 6 {
            load_into_hl(cpu, instruction, second_register_index);
            return (true, 8);
        }

        let mut destination: u8 = 0;
        if first_register_index < 6 {
            let mut destination = cpu.simple_registers[first_register_index as usize];
        }

        if second_register_index < 6 {
            destination = cpu.simple_registers[second_register_index as usize];
            print!("loading register {} into register {}", second_register_index, destination);
            return (true, 4);
        }

        if second_register_index == 6 {
            let address = cpu.read_hl();
            destination = cpu.memory_map.fetch_byte(address as u16);
            return (true, 8);
        }

        return (false, 0);
    }
    (false, 0)
}

fn load_a_ff_c(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xF2 {
        let value = cpu
            .memory_map
            .fetch_byte(0xFF00 + cpu.simple_registers[1] as u16);
        cpu.accumulator = value;
        return (true, 8);
    }
    (false, 0)
}

fn load_ff_c_a(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xE2 {
        let address = 0xFF00 | cpu.simple_registers[1] as u16;
        cpu.memory_map.store_byte(address, cpu.accumulator);
        return (true, 8);
    }
    (false, 0)
}

fn ldh_n_a(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xE0 {
        let address = 0xFF00 | cpu.read_and_advance_program_counter() as u16;
        cpu.memory_map.store_byte(address, cpu.accumulator);
        return (true, 12);
    }
    (false, 0)
}

fn ldh_a_n(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xF0 {
        let address = 0xFF00 | cpu.read_and_advance_program_counter() as u16;
        let value = cpu.memory_map.fetch_byte(address);
        cpu.accumulator = value;
        return (true, 12);
    }
    (false, 0)
}

fn ld_sp_hl(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xF9 {
        let value = cpu.read_hl_address();
        cpu.stack_pointer = value;
        return (true, 8)
    }
    (false, 0)
}

fn load_into_hl(cpu: &mut Cpu, instruction: u8, second_register_index: u32) {
    let destination_address = cpu.read_hl_address();
    if second_register_index < 6 {
        let value = cpu.simple_registers[second_register_index as usize];
        cpu.memory_map.store_byte(destination_address, value);
    }
    if second_register_index == 6 {
        panic!("opcode 76 is halt! read opcode {:x}", instruction);
    }
}

fn compare(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xFE {
        let to = cpu.read_and_advance_program_counter();
        let from = cpu.accumulator.clone();
        subtract_and_set_flags(cpu, from, to);
        return (true, 4);
    }
    (false, 0)
}

fn subtract(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let first_half = get_first_half(instruction);
    let second_half = get_second_half(instruction);
    if first_half == 0x9 && second_half < 0x6 {
        let a = cpu.accumulator;
        let b = cpu.simple_registers[second_half as usize];
        cpu.accumulator = subtract_and_set_flags(cpu, a, b);
        return (true, 4);
    }
    (false, 0)
}



fn xor(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let first_half = get_first_half(instruction);
    let second_half = get_second_half(instruction);
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

fn jump(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xc3 {
        let value = cpu.read_immediate_value_16();
        cpu.program_counter = value;
        return (true, 12);
    }
    (false, 0)
}

fn jump_cc_n(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let mut should_jump = false;
    match instruction {
        0x18 => { should_jump = true; },
        0x20 => { should_jump = !cpu.flags[7]; },
        0x28 => { should_jump = cpu.flags[7]; },
        0x30 => { should_jump = !cpu.flags[4] },
        0x38 => { should_jump = cpu.flags[4] },
        _ => {
            return (false, 0);
        }
    }
    // done here because the immediate value should not be interpreted as an opcode
    let distance = cpu.read_and_advance_program_counter() as i8;
    if should_jump {
        let pc = cpu.program_counter as i16;
        cpu.program_counter = pc.wrapping_add(distance as i16) as u16;
        return (true, 12);
    }
    (true, 8)
}

fn enable_interrupts(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xFB {
        cpu.interrupts_enabled = true;
        return (true, 4);
    }
    (false, 0)
}

fn disable_interrupts(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xF3 {
        cpu.interrupts_enabled = false;
        return (true, 4);
    }
    (false, 0)
}

fn call_immediate_16(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xCD {
        //TODO: first and second might have to be written in reverse;
        let jump_to = cpu.read_immediate_value_16();
        let address = cpu.program_counter;
        cpu.push_stack(address);
        cpu.program_counter = jump_to;
        return (true, 12);
    }
    (false, 0)
}

fn ret(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xC9 {
        do_return(cpu);
        return (true, 8);
    }
    (false, 0)
}

fn ret_cc(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xC0 {
        if !cpu.flags[7] {
            do_return(cpu);
        }
        return (true, 8);
    }
    if instruction == 0xC8 {
        if cpu.flags[7] {
            do_return(cpu);
        }
        return (true, 8);
    }
    if instruction == 0xD0 {
        if !cpu.flags[4] {
            do_return(cpu);
        }
        return (true, 8);
    }
    if instruction == 0xD8 {
        if cpu.flags[4] {
            do_return(cpu);
        }
        return (true, 8);
    }
    (false, 0)
}

pub fn pop(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let mut register_pair = -1;

    let (first_half, second_half) = split_into_halves(instruction);
    if second_half == 1 && first_half >= 0xC {
        let value = cpu.pop_stack();
        cpu.write_combined_register(value, (first_half - 0xC) * 2);
        return (true, 12)
    }
    (false, 0)
}

pub fn push(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half) = split_into_halves(instruction);
    if second_half == 5 && first_half >= 0xC {
        let value = cpu.read_combined_register((first_half - 0xC) * 2);
        cpu.push_stack(value);
        return (true, 16);
    }
    (false, 0)
}

fn increment(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half) = split_into_halves(instruction);

    if first_half <= 3 && (second_half == 0xC || second_half == 0x4){
        match instruction {
            0x3C => {
                let value = cpu.accumulator.clone();
                set_flags_for_increment(cpu, value);
                cpu.accumulator = wrapping_increment(value);
            }
            0x34 => {
                let mut value = cpu.read_hl();
                set_flags_for_increment(cpu, value);
                value = wrapping_increment(value);
                let address = cpu.read_hl_address();
                cpu.memory_map.store_byte(address, value);
                return (true, 12);
            }
            _ => {
                let register= (first_half * 2) + second_half / 8;
                let value = cpu.simple_registers[register as usize];
                set_flags_for_increment(cpu, value);
                let new_value = wrapping_increment(value);
                cpu.simple_registers[register as usize] = new_value;
                return (true, 4);
            }
        }
    }
    (false,0)
}

fn increment_nn(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half) = split_into_halves(instruction);

    if second_half == 3 && first_half <= 3 {
        let first_register = (first_half * 2);
        let value = wrapping_increment_16(cpu.read_combined_register(first_register));
        cpu.write_combined_register(value,first_register);
        return (true, 8);
    }
    (false, 0)
}

fn decrement_n(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half) = split_into_halves(instruction);
    if first_half <= 3 && (second_half == 5 || second_half == 0xD) {
        if instruction == 0x3D {
            let value = cpu.accumulator;
            set_flags_for_decrement(cpu, value);
            cpu.accumulator = wrapping_decrement(value);
        } else if instruction == 0x35 {
            let value = cpu.read_hl();
            set_flags_for_decrement(cpu, value);
            let address = cpu.read_hl_address();
            cpu.memory_map.store_byte(address, wrapping_decrement(value));
            return (true, 12);
        } else {
            let register = first_half * 2 + ((second_half - 5) / 8);
            let mut value = cpu.simple_registers[register as usize];
            set_flags_for_decrement(cpu, value);
            cpu.simple_registers[register as usize] = wrapping_decrement(value);
        }
        return (true, 4);
    }
    (false, 0)
}

fn rla(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0x17 {
        let new_value = rotate_left(cpu.accumulator,cpu);
        cpu.accumulator = new_value;
        return (true,4);
    }

    (false,0)
}

/*************************\
*        PREFIX_CB
**************************/
fn prefix_cb(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    if instruction == 0xCB {
        let new_instruction_code = cpu.read_and_advance_program_counter();
        trace!("about to run cb opcode {:x}\n", new_instruction_code);
        for instruction in PREFIX_CB_INSTRUCTIONS_PIPELINE.iter() {
            let (found_instruction, cycles) = instruction(cpu, new_instruction_code);
            if found_instruction {
                return (true, cycles);
            }
        }
        panic!("unsupported prefix cb opcode {:x}\n", new_instruction_code);
    }
    (false, 0)
}

fn test_bit(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half)= split_into_halves(instruction);
    if first_half >= 4 && first_half <= 7 {
        let bit_index: u8 = (first_half - 4) * 2 + second_half / 8;
        let register_index = second_half % 8;
        let mut bit_is_zero = false;
        let mut cycles = 0;
        if second_half == 6 {
            bit_is_zero = is_bit_zero(cpu.read_hl(), bit_index);
            cycles = 16;
        } else if second_half == 7 {
            bit_is_zero = is_bit_zero(cpu.accumulator, bit_index);
            cycles = 8;
        } else {
            bit_is_zero = is_bit_zero(cpu.simple_registers[register_index as usize], bit_index);
            cycles = 8;
        }
        if cycles != 0 {
            let c = cpu.flags[4];
            cpu.flags = [false,false,false,false,c,true,false,bit_is_zero];
            return (true, cycles);
        }
    }
    (false, 0)
}

fn rl_n(cpu: &mut Cpu, instruction: u8) -> (bool, u8) {
    let (first_half, second_half)= split_into_halves(instruction);
    if first_half == 1 && second_half <= 7 {
        if second_half == 6 {
            let new_value = rotate_left(cpu.read_hl(),cpu);
            let address = cpu.read_hl_address();
            cpu.memory_map.store_byte(address,new_value);
            return (true,16);
        } else if second_half == 7 {
            cpu.accumulator = rotate_left(cpu.accumulator,cpu);
        }else{
            let new_value = rotate_left(cpu.simple_registers[first_half as usize],cpu);
            cpu.simple_registers[first_half as usize] = new_value;
        }
        return (true,8)
    }
    (false, 0)
}

/*************************\
*         HELPERS
**************************/
fn rotate_left(value:u8, cpu:&mut Cpu) -> u8 {
    let new_value = (value << 1)| cpu.flags[4] as u8;
    cpu.flags[4] = (value >> 7) != 0;
    cpu.flags[5] = false;
    cpu.flags[6] = false;
    cpu.flags[7] = new_value == 0;
    new_value
}

fn wrapping_decrement(value: u8) -> u8 {
    let new_value = (Wrapping(value) - Wrapping(1)).0;
    new_value
}

fn wrapping_decrement_16(value: u16) -> u16 {
    let new_value = (Wrapping(value) - Wrapping(1)).0;
    new_value
}

fn wrapping_increment(value: u8) -> u8 {
    let new_value = (Wrapping(value) + Wrapping(1)).0;
    new_value
}

fn wrapping_increment_16(value: u16) -> u16 {
    let new_value = (Wrapping(value) + Wrapping(1)).0;
    new_value
}

fn is_bit_zero(value:u8, bit_index:u8) -> bool {
    false == BitVec::from_bytes(&[value])[7-bit_index as usize]
}

fn set_flags_for_decrement(cpu: &mut Cpu, value: u8) {
    cpu.flags[5] = value & 0xf == 0;
    cpu.flags[6] = true;
    cpu.flags[7] = wrapping_decrement(value) == 0;
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
        wrapping_increment(value) == 0,
    ];
}

fn do_return(cpu: &mut Cpu) {
    let address = cpu.pop_stack();
    cpu.program_counter = address;
}

fn subtract_and_set_flags(cpu: &mut Cpu, a: u8, b: u8) -> u8 {
    // Check for borrow using 32bit arithmetics
    let a = a as u32;
    let b = b as u32;

    let r = a.wrapping_sub(b);

    let rb = r as u8;

    cpu.flags = [
        false,
        false,
        false,
        false,
        r & 0x100 != 0,
        (a ^ b ^ r) & 0x10 != 0,
        true,
        rb == 0,
    ];
    rb
}

fn to_u32(slice: &[u8]) -> u32 {
    slice.iter().rev().fold(0, |acc, &b| acc * 2 + b as u32)
}

fn split_into_halves(byte: u8) -> (u8, u8) {
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