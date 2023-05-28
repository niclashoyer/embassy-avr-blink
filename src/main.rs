#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(abi_avr_interrupt)]

mod embassy;

use arduino_hal::{
    hal::port::{PD0, PD1},
    pac::USART0,
    port::{
        mode::{Input, Output},
        Pin,
    },
    prelude::*,
};
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_time::{Duration, Timer};
use panic_halt as _;
use simavr_section::{avr_mcu, avr_mcu_vcd_trace};

avr_mcu!(16000000, b"atmega328p");
avr_mcu_vcd_trace!(AVR_MMCU_TAG_VCD_PORTPIN, b'B', 5, b"PINB5");

type Usart = arduino_hal::usart::Usart<USART0, Pin<Input, PD0>, Pin<Output, PD1>>;

#[embassy_executor::task]
async fn talk(mut serial: Usart) -> ! {
    loop {
        ufmt::uwriteln!(&mut serial, "Hello from Arduino!\r").void_unwrap();
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    embassy::init(dp.TC0);

    unsafe { avr_device::interrupt::enable() };

    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);

    spawner.spawn(talk(serial)).unwrap();

    let mut led = pins.d13.into_output();

    loop {
        led.toggle();
        Timer::after(Duration::from_millis(250)).await;
    }
}
