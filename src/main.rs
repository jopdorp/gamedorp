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
extern crate ini;
extern crate num;
extern crate sdl2;
extern crate time;
#[cfg(windows)]
extern crate winapi;

mod cartridge;
mod cpu;
mod gb_rs_cpu;
mod gpu;
mod io;
mod resampler;
mod spu;
mod ui;
mod emulator;

use std::path::Path;
use emulator::Emulator;

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

    let mut emulator = Emulator::new();

    let rompath = Path::new(&argv[1]);
    emulator.start_emulation(rompath);
}