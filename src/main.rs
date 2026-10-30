mod fire;
mod stats;

use std::env;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::process;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};

use fire::{Fire, MAX_HEAT};
use stats::{Cpu, Thermometer};

const SAMPLE_EVERY: Duration = Duration::from_millis(250);

const USAGE: &str = "\
usage: brazier [--total] [--fps N] [--info]

  --total    one fire for the whole CPU instead of one per core
  --fps N    frames per second, 1 to 120 (default 30)
  --info     show load and temperature from the start

keys: q or Esc quits, i toggles the readout";

struct Options {
    total: bool,
    fps: u32,
    info: bool,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
        let mut opts = Options { total: false, fps: 30, info: false };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--total" => opts.total = true,
                "--info" => opts.info = true,
                "--fps" => {
                    let value = args.next().ok_or("--fps needs a number")?;
                    opts.fps = match value.parse() {
                        Ok(n @ 1..=120) => n,
                        _ => return Err(format!("bad --fps value: {value}")),
                    };
                }
                "-h" | "--help" => {
                    println!("{USAGE}");
                    process::exit(0);
                }
                "-V" | "--version" => {
                    println!("brazier {}", env!("CARGO_PKG_VERSION"));
                    process::exit(0);
                }
                _ => return Err(format!("unknown argument: {arg}\n\n{USAGE}")),
            }
        }
        Ok(opts)
    }
}

// Restores the terminal when dropped, so a panic doesn't leave it in raw mode.
struct Screen;

impl Screen {
    fn enter() -> io::Result<Screen> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
        Ok(Screen)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn main() {
    let opts = Options::parse(env::args().skip(1)).unwrap_or_else(|msg| {
        eprintln!("brazier: {msg}");
        process::exit(2);
    });
    if let Err(err) = run(&opts) {
        eprintln!("brazier: {err}");
        process::exit(1);
    }
}

fn run(opts: &Options) -> io::Result<()> {
    let mut cpu = Cpu::open()?;
    let thermometer = Thermometer::find();

    let _screen = Screen::enter()?;
    let mut out = io::stdout().lock();
    let (cols, rows) = terminal::size()?;
    let mut fire = Fire::new(cols as usize, rows as usize * 2);
    let mut frame = String::new();
    let mut show_info = opts.info;

    let frame_time = Duration::from_secs(1) / opts.fps;
    let mut load = cpu.sample()?;
    let mut temp = thermometer.read();
    let mut levels: Vec<f32> = Vec::new();
    let mut next_sample = Instant::now() + SAMPLE_EVERY;
    let mut last = Instant::now();

    loop {
        let now = Instant::now();
        if now >= next_sample {
            load = cpu.sample()?;
            temp = thermometer.read();
            next_sample = now + SAMPLE_EVERY;
        }
        let dt = (now - last).as_secs_f32();
        last = now;

        let targets = if opts.total { std::slice::from_ref(&load.total) } else { &load.cores[..] };
        levels.resize(targets.len(), 0.0);
        for (level, &target) in levels.iter_mut().zip(targets) {
            // flare up fast, die down slowly
            let lag = if target > *level { 0.15 } else { 0.8 };
            *level += (target - *level) * (1.0 - (-dt / lag).exp());
        }

        fire.step(base_heat(temp), &levels);
        frame.clear();
        fire.draw(&mut frame);
        if show_info {
            let _ = write!(frame, "\x1b[1;2H\x1b[38;2;150;150;150mcpu {:3.0}%", load.total * 100.0);
            if let Some(t) = temp {
                let _ = write!(frame, "  {t:.0}°C");
            }
            frame.push_str("\x1b[0m");
        }
        out.write_all(frame.as_bytes())?;
        out.flush()?;

        let deadline = now + frame_time;
        while let Some(wait) = deadline.checked_duration_since(Instant::now()) {
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                    KeyCode::Char('i') => show_info = !show_info,
                    _ => {}
                },
                Event::Resize(cols, rows) => fire = Fire::new(cols as usize, rows as usize * 2),
                _ => {}
            }
        }
    }
}

// The colder the CPU, the cooler the bottom of the flame:
// orange at 40 °C and below, white at 90 °C and above.
fn base_heat(temp: Option<f32>) -> u8 {
    let Some(t) = temp else { return 30 };
    let k = ((t - 40.0) / 50.0).clamp(0.0, 1.0);
    22 + (k * (MAX_HEAT - 22) as f32).round() as u8
}
