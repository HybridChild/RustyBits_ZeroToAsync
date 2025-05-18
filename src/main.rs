#![no_std]
#![no_main]

mod ticker;
use ticker::Ticker;

mod led;
use led::LedTask;

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
    let mut user_led = cortex_m::interrupt::free(|cs| gpioa.pa5.into_push_pull_output(cs));
    // Turn on LED to indicate program has started
    user_led.set_high().unwrap();

    let ticker = Ticker::new(dp.TIM2, &mut rcc);

    let mut led_task = LedTask::new(user_led, &ticker);
    
    // Main loop
    loop {
        led_task.poll();
    }
}
