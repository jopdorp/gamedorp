use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use std::path::Path;
use ini::Ini;
#[cfg(windows)] use winapi::um::processthreadsapi::{GetCurrentProcess, SetThreadPriority};

use cartridge::Cartridge;
use cpu::CanRunInstruction;
use ui::sdl2::{Audio, Context};
use ui::{Event, AudioTrait};
use spu::Spu;
use gpu::Gpu;
use io::Interconnect;
use cpu::Cpu;
use spu::samples_per_steps;

pub struct Emulator {
    sdl2: Context,
}

impl Emulator {
    pub fn new() -> Emulator {
        Emulator {
            sdl2: Context::new(),
        }
    }

    pub fn start_emulation(&mut self, rom_path: &Path) -> () {
        let cartridge = match Cartridge::from_path(&rom_path) {
            Ok(r) => r,
            Err(e) => panic!("Failed to load ROM: {}", e),
        };

        let (spu, audio_channel) = Spu::new();
        let mut audio = Audio::new(audio_channel, &self.sdl2.sdl2);
        audio.start();

        let (upscale, fullscreen, cpu_plugin) = read_config();

        let mut display = self.sdl2.new_display(upscale, fullscreen);
        let gpu = Gpu::new(&mut display);
        let inter = Interconnect::new(cartridge, gpu, spu, self.sdl2.buttons());

        let mut cpu: Box<CanRunInstruction> = if cpu_plugin == "gb-rs" {
            Box::new(::gb_rs_cpu::Cpu::new(inter))
        } else {
            Box::new(Cpu::new(inter))
        };

        let tick_rx = start_sending_sync_ticks();

        let mut audio_adjust_count = 0;
        let mut cycles: u64 = 0;

        loop {
            while cycles < ::INSTRUCTIONS_BETWEEN_TICKS {
                // The actual emulator takes place here!
                cycles += cpu.run_next_instruction() as u64;
            }
            cycles -= ::INSTRUCTIONS_BETWEEN_TICKS;
            // Update controller status
            match self.sdl2.update_buttons() {
                Event::PowerOff => break,
                Event::None => (),
            }
            // Sleep until next batch cycle
            if let Err(e) = tick_rx.recv() {
                panic!("Timer died: {:?}", e);
            }
            audio_adjust_count += ::INSTRUCTIONS_BETWEEN_TICKS;
            if audio_adjust_count
                >= ::GAMEBOY_SYSTEM_CLOCK_FREQUENCY * ::AUDIO_RESAMPLING_ADJUST_DELAY_SECONDS
                {
                    // Retrieve the number of samples generated since the last
                    // adjustment
                    let s = samples_per_steps(audio_adjust_count as u32);
                    audio.adjust_resampling(s);
                    audio_adjust_count = 0;
                }
        }
    }
}

fn read_config() -> (u8, bool, String) {
    let conf = Ini::load_from_file("config.ini").unwrap();
    let section = conf.section(Some("display")).unwrap();
    let upscale = section.get("upscale").unwrap().parse::<u8>().unwrap();
    let fullscreen = section.get("fullscreen").unwrap().parse::<bool>().unwrap();
    let section = conf.section(Some("plugins")).unwrap();
    let cpu_plugin = section.get("cpu").unwrap().parse::<String>().unwrap();
    (upscale, fullscreen, cpu_plugin)
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
    let batch_duration = (::INSTRUCTIONS_BETWEEN_TICKS as f64
        * (1_000_000_000 as f64 / ::GAMEBOY_SYSTEM_CLOCK_FREQUENCY as f64))
        as u64;
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
                } else {
                    last_time = current_time;
                }
                if let Err(_) = tick_tx.send(()) {
                    // End thread
                    return;
                }
            } else {
                // sleep for at least 1/8 of a batch duration
                std::thread::sleep(Duration::new(
                    0,
                    ((batch_duration - duration_since_last_tick) / 4) as u32,
                ));
            }
        }
    });
    tick_rx
}
