#[cfg(test)]
mod test {
    use std::fmt::{Debug,Formatter, Error};
    use std::thread::sleep;
    use std::time::Duration;
    use std::path::Path;


    pub use cpu::Cpu;
    pub use cpu::CanRunInstruction;

    #[test]
    fn cpu_has_the_same_states_as_gb_rs_cpu() {
        let rompath = Path::new("Tetris.gb");

        let cart = match ::cartridge::Cartridge::from_path(&rompath) {
            Ok(r)  => r,
            Err(e) => panic!("Failed to load ROM: {}", e),
        };

        let sdl2 = ::ui::sdl2::Context::new();
        let mut display = sdl2.new_display(1);
        let gpu = ::gpu::Gpu::new(&mut display);
        let (spu, audio_channel) = ::spu::Spu::new();
        let mut audio = ::ui::sdl2::Audio::new(audio_channel, &sdl2.sdl2);
        audio.start();
        let inter = ::io::Interconnect::new(cart, gpu, spu, sdl2.buttons());
        let mut cpu = ::cpu::Cpu::new(inter);

        let cart2 = match ::cartridge::Cartridge::from_path(&rompath) {
            Ok(r)  => r,
            Err(e) => panic!("Failed to load ROM: {}", e),
        };

        let mut display2 = sdl2.new_display(1);
        let gpu2 = ::gpu::Gpu::new(&mut display2);
        let (spu2, audio_channel2) = ::spu::Spu::new();
        let mut audio2 = ::ui::sdl2::Audio::new(audio_channel2, &sdl2.sdl2);
        audio2.start();
        let inter2 = ::io::Interconnect::new(cart2, gpu2, spu2, sdl2.buttons());
        let mut cpu2 = ::gb_rs_cpu::Cpu::new(inter2);


        while cpu2.regs.pc != 0x897987 {
            let pc = cpu.program_counter;
            let pc2 = cpu2.regs.pc;
            print!("cpu1 testing instruction 0x{:x} for pc 0x{:x}\n", cpu.memory_map.fetch_byte(pc), pc);
            print!("cpu2 testing instruction 0x{:x} for pc 0x{:x}\n", cpu2.inter.fetch_byte(pc2), pc2);
            cpu.run_next_instruction();
            cpu2.run_next_instruction();
            print!("next cpu1 pc {:x}\n", cpu.program_counter);
            print!("next cpu2 pc {:x}\n", cpu2.regs.pc);
            if cpu2.regs.pc == 0x42 ||
                cpu2.regs.pc == 0x45 ||
                cpu2.regs.pc == 0x4B {

                let flat_cpu1 = flatten(&cpu);
                let flat_cpu2 = flatten_gr_rs(&cpu2);
                assert_eq!(flat_cpu1,flat_cpu2);
                for i in 0..0xFFFF {
                    if i == 0xff04 {
                        // cycles for instructions are not identical.
                        // cpu1 uses cycles as reported in:
                        // http://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html
                        continue;
                    }
                    let gamedorp_val = cpu.memory_map.fetch_byte(i);
                    let gb_rs_val = cpu2.inter.fetch_byte(i);
                    if  gamedorp_val != gb_rs_val {
                        print!("memory is different at 0x{:x} gamedorp has {:x} gb_rs has {:x}\n",i,gamedorp_val,gb_rs_val);
                    }
                    assert_eq!(gamedorp_val,gb_rs_val)
                }
            }
        }
    }

    fn flatten(cpu: &Cpu) -> (u8, u16, u16, [bool;8], [u8;6]) {
        (cpu.accumulator, cpu.program_counter, cpu.stack_pointer, cpu.flags.clone(), cpu.simple_registers.clone())
    }


    fn flatten_gr_rs(cpu: &::gb_rs_cpu::Cpu) -> (u8, u16, u16, [bool;8], [u8;6]) {
        let flags = [false,false,false,false,cpu.flags.c,cpu.flags.h,cpu.flags.n,cpu.flags.z];
        let h = ((cpu.regs.hl & 0xFF00) >> 8) as u8;
        let l = (cpu.regs.hl & 0xFF) as u8;
        (cpu.regs.a, cpu.regs.pc, cpu.regs.sp, flags, [cpu.regs.b,cpu.regs.c,cpu.regs.d,cpu.regs.e,h,l])
    }
}