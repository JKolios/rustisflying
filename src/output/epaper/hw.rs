//! Hardware output to the Waveshare 2.7inch e-Paper HAT (B) on the Pi's
//! GPIO header (SPI0, CE0; BCM 24/25/17 for BUSY/DC/RST).
//!
//! A full tri-color refresh takes seconds, so the driver lives on a
//! dedicated worker thread: [`FlightOutput::emit`] just records the latest
//! tick (latest wins) and wakes the worker, which renders, skips the refresh
//! when the frame is unchanged, and otherwise pushes both planes and puts
//! the panel back to sleep.

use super::super::FlightOutput;
use super::{layout, render};
use crate::model::TickResult;
use anyhow::{Context, Result};
use epd_waveshare::{epd2in7b::Epd2in7b, prelude::*};
use linux_embedded_hal::{
    gpio_cdev::{Chip, LineRequestFlags},
    spidev::{SpiModeFlags, SpidevOptions},
    CdevPin, Delay, SpidevDevice,
};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

/// Standard HAT wiring (BCM pin numbers).
const PIN_BUSY: u32 = 24;
const PIN_DC: u32 = 25;
const PIN_RST: u32 = 17;
/// The HAT sits on SPI0 with CE0.
const SPI_DEVICE: &str = "/dev/spidev0.0";
const GPIO_CHIP: &str = "/dev/gpiochip0";
/// 4 MHz, MODE 0 — within the panel controller's limits.
const SPI_HZ: u32 = 4_000_000;

type Panel = Epd2in7b<SpidevDevice, CdevPin, CdevPin, CdevPin, Delay>;

/// Shared between the emitting task and the worker thread: the latest tick
/// to display, if any.
type Shared = Arc<(Mutex<Option<TickResult>>, Condvar)>;

pub struct EpaperOutput {
    shared: Shared,
}

impl EpaperOutput {
    /// Open SPI + GPIO, clear the panel to white, and start the worker.
    pub fn new() -> Result<Self> {
        let mut spi =
            SpidevDevice::open(SPI_DEVICE).with_context(|| format!("opening {SPI_DEVICE}"))?;
        spi.0
            .configure(
                &SpidevOptions::new()
                    .bits_per_word(8)
                    .max_speed_hz(SPI_HZ)
                    .mode(SpiModeFlags::SPI_MODE_0)
                    .build(),
            )
            .context("configuring SPI")?;

        let mut chip = Chip::new(GPIO_CHIP).with_context(|| format!("opening {GPIO_CHIP}"))?;
        let busy = CdevPin::new(
            chip.get_line(PIN_BUSY)?
                .request(LineRequestFlags::INPUT, 0, "rustisflying")?,
        )?;
        let dc = CdevPin::new(
            chip.get_line(PIN_DC)?
                .request(LineRequestFlags::OUTPUT, 0, "rustisflying")?,
        )?;
        let rst = CdevPin::new(
            chip.get_line(PIN_RST)?
                .request(LineRequestFlags::OUTPUT, 0, "rustisflying")?,
        )?;

        let mut delay = Delay;
        let mut epd = Epd2in7b::new(&mut spi, busy, dc, rst, &mut delay, None)
            .context("initializing the e-paper driver")?;
        // Start from a clean white panel rather than whatever the last run
        // left behind.
        epd.clear_frame(&mut spi, &mut delay).context("clearing the panel")?;
        epd.display_frame(&mut spi, &mut delay)
            .context("flushing the initial clear")?;
        epd.sleep(&mut spi, &mut delay).context("sleeping the panel")?;

        let shared: Shared = Arc::new((Mutex::new(None), Condvar::new()));
        let worker_shared = shared.clone();
        thread::Builder::new()
            .name("epaper".into())
            .spawn(move || worker(worker_shared, spi, epd, delay))
            .context("spawning the epaper worker")?;

        Ok(Self { shared })
    }
}

impl FlightOutput for EpaperOutput {
    fn emit(&self, result: &TickResult) {
        let (lock, condvar) = &*self.shared;
        *lock.lock().unwrap() = Some(result.clone());
        condvar.notify_one();
    }
}

/// Block until a new tick lands in the shared slot (latest wins: a tick
/// emitted while the worker is busy replaces the pending one).
fn next_tick(shared: &Shared) -> TickResult {
    let (lock, condvar) = &**shared;
    let mut pending = lock.lock().unwrap();
    loop {
        if let Some(result) = pending.take() {
            return result;
        }
        pending = condvar.wait(pending).unwrap();
    }
}

fn worker(shared: Shared, mut spi: SpidevDevice, mut epd: Panel, mut delay: Delay) {
    // The last frame actually pushed, so unchanged content skips the
    // (slow, panel-wearing) refresh entirely.
    let mut last_frame: Option<Vec<u8>> = None;
    loop {
        let result = next_tick(&shared);
        let screen = layout::layout(&result);
        let planes = render::render(&screen);
        let mut frame = Vec::with_capacity(planes.black.buffer().len() + planes.chromatic.buffer().len());
        frame.extend_from_slice(planes.black.buffer());
        frame.extend_from_slice(planes.chromatic.buffer());
        if last_frame.as_deref() == Some(&frame) {
            continue;
        }
        match refresh(&mut spi, &mut epd, &mut delay, &frame) {
            Ok(()) => last_frame = Some(frame),
            Err(e) => eprintln!("epaper: refresh failed: {e:#}"),
        }
    }
}

/// Full update cycle: wake, push both planes, refresh, sleep. E-paper is
/// bistable — the image persists through deep sleep, which only cuts the
/// panel's idle power draw — so every refresh ends by sleeping the panel
/// until the next changed frame.
fn refresh(spi: &mut SpidevDevice, epd: &mut Panel, delay: &mut Delay, planes: &[u8]) -> Result<()> {
    let (black, chromatic) = planes.split_at(planes.len() / 2);
    epd.wake_up(spi, delay).context("waking the panel")?;
    epd.update_color_frame(spi, delay, black, chromatic)
        .context("sending the frame")?;
    epd.display_frame(spi, delay).context("refreshing")?;
    epd.sleep(spi, delay).context("sleeping the panel")?;
    Ok(())
}
