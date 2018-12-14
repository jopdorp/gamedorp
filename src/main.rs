//! gb-rs: Game Boy emulator
//! Ressources:
//!
//! Opcode map: http://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html
//! JS emulator: http://imrannazar.com/GameBoy-Emulation-in-JavaScript:-The-CPU
//! Lots of info about GC quircks: http://www.devrs.com/gb/files/faqs.html
//! Accuracy tests: http://tasvideos.org/EmulatorResources/GBAccuracyTests.html
#![warn(missing_docs)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

extern crate ascii;
extern crate num;
extern crate sdl2;
extern crate time;
#[cfg(windows)] extern crate winapi;

use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
#[cfg(windows)] use winapi::um::processthreadsapi::{GetCurrentProcess, SetThreadPriority};

use cpu::CanRunInstruction;
use ui::Audio;

mod cartridge;
mod cpu;
mod gb_rs_cpu;
mod gpu;
mod io;
mod resampler;
mod spu;
mod ui;


const INSTRUCTIONS_BETWEEN_TICKS: u64 = 0x2000;
const GAMEBOY_SYSTEM_CLOCK_FREQUENCY: u64 = 0x400000;
const AUDIO_RESAMPLING_ADJUST_DELAY_SECONDS: u64 = 1;


#[allow(dead_code)]
fn main() {
    let argv: Vec<_> = std::env::args().collect();

    if argv.len() < 2 {
        print!("Usage: {} <rom-file>\n", argv[0]);
        return;
    }

    let rompath = Path::new(&argv[1]);

    let cart = match cartridge::Cartridge::from_path(&rompath) {
        Ok(r) => r,
        Err(e) => panic!("Failed to load ROM: {}", e),
    };

    print!("Loaded ROM {:?}\n", cart);

    let sdl2 = ui::sdl2::Context::new();
    let mut display = sdl2.new_display(5, true);
    let gpu = gpu::Gpu::new(&mut display);
    let (spu, audio_channel) = spu::Spu::new();
    let mut audio = ui::sdl2::Audio::new(audio_channel, &sdl2.sdl2);
    audio.start();
    let inter = io::Interconnect::new(cart, gpu, spu, sdl2.buttons());

    let mut cpu: Box<CanRunInstruction> = if argv.len() > 2 && &argv[2] == "gb-rs" {
        Box::new(::gb_rs_cpu::Cpu::new(inter))
    } else {
        Box::new(cpu::Cpu::new(inter))
    };

    let tick_rx = start_sending_sync_ticks();

    let mut audio_adjust_count = 0;
    let mut cycles: u64 = 0;

    loop {
        while cycles < INSTRUCTIONS_BETWEEN_TICKS {
            // The actual emulator takes place here!
            cycles += cpu.run_next_instruction() as u64;
        }
        cycles -= INSTRUCTIONS_BETWEEN_TICKS;
        // Update controller status
        match sdl2.update_buttons() {
            ui::Event::PowerOff => break,
            ui::Event::None => (),
        }
        // Sleep until next batch cycle
        if let Err(e) = tick_rx.recv() {
            panic!("Timer died: {:?}", e);
        }
        audio_adjust_count += INSTRUCTIONS_BETWEEN_TICKS;
        if audio_adjust_count >= GAMEBOY_SYSTEM_CLOCK_FREQUENCY * AUDIO_RESAMPLING_ADJUST_DELAY_SECONDS {
            // Retrieve the number of samples generated since the last
            // adjustment
            let s = spu::samples_per_steps(audio_adjust_count as u32);
            audio.adjust_resampling(s);
            audio_adjust_count = 0;
        }
    }
}

// In order to synchronize the emulation speed with the wall clock
// we need to wait at some point so that we don't go too
// fast. Waiting between each cycle would mean a storm of syscalls
// back and forth between the kernel and us, so instead we execute
// instructions in batches of GRANULARITY cycles and then sleep
// for a while. If the GRANULARITY value is too low we'll go to
// sleep very often which will have poor performance. If it's too
// high it might look like the emulation is stuttering.
fn start_sending_sync_ticks() -> Receiver<()> {
    let batch_duration = (INSTRUCTIONS_BETWEEN_TICKS as f64 * (1_000_000_000 as f64 / GAMEBOY_SYSTEM_CLOCK_FREQUENCY as f64)) as u64;
    let (tick_tx, tick_rx) = channel();

    ::std::thread::spawn(move || {
        #[cfg(windows)]
            unsafe {
            let process = GetCurrentProcess();
            SetThreadPriority(process, 0x00010000);
        }
        let mut last_time = time::precise_time_ns();
        loop {
            let current_time = time::precise_time_ns();
            let duration_since_last_tick = current_time - last_time;
            if duration_since_last_tick > batch_duration {
                if duration_since_last_tick < 2 * batch_duration {
                    last_time = last_time + batch_duration;
                }else{
                    last_time = current_time;
                }
                if let Err(_) = tick_tx.send(()) {
                    // End thread
                    return;
                }
            } else {
                // sleep for at least 1/8 of a batch duration
                std::thread::sleep(Duration::new(0, ((batch_duration - duration_since_last_tick) / 4) as u32));
            }
        }
    });
    tick_rx
}