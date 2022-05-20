#![no_std]
#![no_main]

use core::cell::Cell;
use core::convert::Infallible;

use crate::dithering_adaptor::DitheringAdaptor;
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
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::DrawTarget;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X9, MonoTextStyleBuilder},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_hal::digital::v2::{InputPin, OutputPin, ToggleableOutputPin};
use embedded_time::fixed_point::FixedPoint;
use embedded_time::rate::Hertz;
use panic_persist as _;
use sh1106::prelude::*;

mod dithering_adaptor;
mod panic_display;

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
        Hertz::new(133_000_000u32),
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
    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS);

    cortex_m::interrupt::free(|cs| {
        SYSTICK_STATE
            .borrow(cs)
            .set(Some(pins.led.into_push_pull_output()))
    });

    //2Hz
    let reload_value = 500_000 - 1;
    core.SYST.set_reload(reload_value);
    core.SYST.clear_current();
    //External clock, driven by the Watchdog - 1 tick per us
    core.SYST.set_clock_source(SystClkSource::External);
    core.SYST.enable_interrupt();
    core.SYST.enable_counter();

    //let mut rng = SmallRng::seed_from_u64(12345);
    const FRAME_TIME: u64 = 20_010;

    //full frame
    //21 down
    //20 down v.slow
    //19_950 down v.slow
    //19_930 down (sometimes up)
    //19_925 up
    //19_900 up
    //19_750 up
    //19 up

    //line draw

    //20_100 - 1 down fast
    //20_020 - 1 down very slow
    //20_019 - down very slow
    //20_017 - down very slow
    //20_016 - down very very slow
    //20_010 - 1 up slow
    //20_000 - 1 up slow
    //19_950 - 1 up
    //19_930 - 1 up

    let p1 = pins.key1.into_pull_up_input();
    let p2 = pins.key2.into_pull_up_input();

    let mut next = timer.get_counter() + FRAME_TIME + 18_000;

    let mut display = DitheringAdaptor { display, frame: 0 };

    loop {
        display.clear();
        if timer.get_counter() < next {
            continue;
        }

        draw(&mut display);

        display.flush().unwrap();

        if p1.is_low().unwrap() {
            next -= 10;
        }
        if p2.is_low().unwrap() {
            next += 10;
        }
    }
}

fn draw<D>(display: &mut DitheringAdaptor<D, Infallible>)
where
    D: DrawTarget<Color = BinaryColor, Error = Infallible>,
{
    let shape_fill = Gray8::new(64);
    let text = Gray8::new(224);
    let background = Gray8::new(32);

    Circle::new(Point::new(0, 0), 41)
        .into_styled(PrimitiveStyle::with_fill(shape_fill))
        .draw(display)
        .unwrap();

    Rectangle::new(Point::new(20, 20), Size::new(80, 60))
        .into_styled(PrimitiveStyle::with_fill(shape_fill))
        .draw(display)
        .unwrap();

    // Can also be written in the shorter form: TextStyle::new(&FONT_6X9, Rgb565::WHITE)
    let no_background = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(Gray8::new(255))
        .build();

    let filled_background = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(text)
        .background_color(background)
        .build();

    let inverse_background = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(background)
        .background_color(text)
        .build();

    Text::new(
        "Hello world! - no background",
        Point::new(15, 15),
        no_background,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "Hello world! - filled background",
        Point::new(15, 30),
        filled_background,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "Hello world! - inverse background",
        Point::new(15, 45),
        inverse_background,
    )
    .draw(display)
    .unwrap();
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
