#![no_std]
#![no_main]

use core::cell::Cell;
use core::convert::Infallible;

use adafruit_macropad::{
    hal::{
        self as hal,
        clocks::Clock as _,
        pac::{self},
    },
    Pins,
};
use cortex_m::interrupt::Mutex;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::{entry, exception};
use embedded_hal::digital::v2::{InputPin, OutputPin, ToggleableOutputPin};
use embedded_time::fixed_point::FixedPoint;
use embedded_time::rate::Hertz;
use embedded_time::Clock;
use panic_persist as _;
use rand::prelude::SmallRng;
use rand::{RngCore, SeedableRng};
use sh1106::prelude::*;

use crate::time::TimerClock;

mod panic_display;
mod time;

pub const XOSC_CRYSTAL_FREQ: Hertz = Hertz(12_000_000);

type LedPin = hal::gpio::Pin<hal::gpio::pin::bank0::Gpio13, hal::gpio::PushPullOutput>;

pub type DynInputPin = dyn InputPin<Error = Infallible>;

static SYSTICK_STATE: Mutex<Cell<Option<LedPin>>> = Mutex::new(Cell::new(None));

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut core = pac::CorePeripherals::take().unwrap();

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        crate::XOSC_CRYSTAL_FREQ.integer(),
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = hal::Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    //display spi
    // These are implicitly used by the spi driver if they are in the correct mode
    let _spi_sclk = pins.sclk.into_mode::<hal::gpio::FunctionSpi>();
    let _spi_mosi = pins.mosi.into_mode::<hal::gpio::FunctionSpi>();
    let _spi_miso = pins.miso.into_mode::<hal::gpio::FunctionSpi>();
    let spi = hal::spi::Spi::<_, _, 8>::new(pac.SPI1);

    // Display control pins
    let oled_dc = pins.oled_dc.into_push_pull_output();
    let oled_cs = pins.oled_cs.into_push_pull_output();
    let mut oled_reset = pins.oled_reset.into_push_pull_output();

    oled_reset.set_high().unwrap(); //disable screen reset

    // Exchange the uninitialised SPI driver for an initialised one
    let oled_spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        Hertz::new(16_000_000u32),
        &embedded_hal::spi::MODE_0,
    );

    let mut display: GraphicsMode<_> = sh1106::Builder::new()
        .connect_spi(oled_spi, oled_dc, oled_cs)
        .into();
    display.init().unwrap();
    display.flush().unwrap();

    if let Some(msg) = panic_persist::get_panic_message_utf8() {
        //NB never returns
        panic_display::display_and_reboot(msg, display, &pins.button.into_pull_up_input());
    }
    static mut CLOCK: Option<TimerClock> = None;
    //Safety: interrupts not enabled yet
    let clock = unsafe {
        CLOCK = Some(TimerClock::new(hal::Timer::new(pac.TIMER, &mut pac.RESETS)));
        CLOCK.as_ref().unwrap()
    };

    cortex_m::interrupt::free(|cs| {
        SYSTICK_STATE
            .borrow(cs)
            .set(Some(pins.led.into_push_pull_output()))
    });

    //100 mico seconds
    // let reload_value = (clocks.system_clock.freq() / 10_000).integer() - 1;
    let reload_value = 1_000 - 1;
    core.SYST.set_reload(reload_value);
    core.SYST.clear_current();
    //External clock, driven by the Watchdog - 1 tick per us
    core.SYST.set_clock_source(SystClkSource::External);
    core.SYST.enable_interrupt();
    core.SYST.enable_counter();

    let mut rng = SmallRng::seed_from_u64(12345);

    let mut rnd = [0u8; 1024];
    for i in 0..rnd.len() {
        rnd[i] = u8::try_from(rng.next_u32() & 0xF).unwrap();
    }

    let mut f = 0;
    let mut r = rng.next_u32();

    loop {
        let _now = clock.try_now().unwrap();
        //display.clear();

        for y in 0..64 {
            for x in 0..128 {
                if x % 8 == 0 {
                    r = rng.next_u32();
                }

                display.set_pixel(
                    x,
                    y,
                    if x == f {
                        0
                    } else {
                        value(x / 16, u8::try_from(r >> x % 8 * 4 & 0x7).unwrap())
                    },
                );
            }
        }

        display.flush().unwrap();
        f = (f + 1) % 128;
    }
}

fn value(x: u32, rng: u8) -> u8 {
    // u8::try_from(x % 2).unwrap()
    if x + u32::from(rng) < 8 {
        0
    } else {
        1
    }
}

#[allow(non_snake_case)]
#[exception]
fn SysTick() {
    static mut LED: Option<LedPin> = None;

    if LED.is_none() {
        *LED = cortex_m::interrupt::free(|cs| SYSTICK_STATE.borrow(cs).take());
    }

    if let Some(led) = LED {
        led.toggle().unwrap();
    }
    cortex_m::asm::sev();
}
