#![no_std]
#![no_main]

mod interrupts;
use interrupts::{config_timer, G_COUNTER};

use cortex_m::interrupt::free;
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

    config_timer(dp.TIM2, &mut rcc);
    
    // Main loop
    let mut last_count = 0;
    loop {
        // Get the current counter value
        let current = free(|cs| *G_COUNTER.borrow(cs).borrow());
        
        // If counter has incremented by 1000 (1 second), toggle LED
        if current >= last_count + 1000 {
            led.toggle().unwrap();
            last_count = current;
        }
        
        // Sleep to save power
        cortex_m::asm::wfi();
    }
}
