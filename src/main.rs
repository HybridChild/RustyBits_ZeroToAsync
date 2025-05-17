#![no_std]
#![no_main]

mod ticker;
use ticker::{Ticker, TickTimer};

use fugit::MillisDuration;
use cortex_m_rt::entry;
use panic_halt as _;
use stm32f0xx_hal::{
    pac,
    prelude::*,
};

#[entry]
fn main() -> ! {
    // Get access to the device peripherals
    let mut dp = pac::Peripherals::take().unwrap();
    
    // Configure the clock system
    let mut rcc = dp.RCC.configure().freeze(&mut dp.FLASH);
    
    // Get access to the GPIO A peripheral
    let gpioa = dp.GPIOA.split(&mut rcc);
    // Configure PA1 as push-pull output for LED (assuming there's an LED on PA1)
    let mut led = cortex_m::interrupt::free(|cs| gpioa.pa5.into_push_pull_output(cs));
    // Turn on LED to indicate program has started
    led.set_high().unwrap();

    let ticker = Ticker::new(dp.TIM2, &mut rcc);
    let duration = MillisDuration::<u32>::from_ticks(500);
    
    // Main loop
    loop {
        let mut tick_timer = TickTimer::new(duration, &ticker);
        tick_timer.start();

        while !tick_timer.is_ready() {
        }

        led.toggle().unwrap();
    }
}
