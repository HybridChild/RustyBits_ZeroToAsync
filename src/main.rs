#![no_std]
#![no_main]

use core::cell::RefCell;
use core::ops::DerefMut;
use cortex_m::interrupt::{free, Mutex};
use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use panic_halt as _;
use stm32f0xx_hal::{
    pac::{self, interrupt, Interrupt, TIM2},
    prelude::*,
    time::Hertz,
    timers::{Event, Timer},
};

// Global variable to share the timer interrupt data
static G_TIM2: Mutex<RefCell<Option<Timer<TIM2>>>> = Mutex::new(RefCell::new(None));
// Counter to track interrupt occurrences
static G_COUNTER: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));

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
    
    // Configure TIM2 for 1ms interrupts (1000 Hz)
    let mut timer = Timer::tim2(dp.TIM2, Hertz(1000), &mut rcc);
    
    // Enable the timer interrupt
    timer.listen(Event::TimeOut);
    
    // Move timer into the global mutex
    free(|cs| {
        G_TIM2.borrow(cs).replace(Some(timer));
    });
    
    // Enable TIM2 interrupt in the NVIC
    unsafe {
        NVIC::unmask(Interrupt::TIM2);
    }
    
    // Main loop
    let mut last_count = 0;
    loop {
        // Get the current counter value
        let current = free(|cs| *G_COUNTER.borrow(cs).borrow());
        
        // If counter has incremented by 1000 (1 second), toggle LED
        if current >= last_count + 1000 {
            led.toggle().unwrap();
            last_count = current;
            
            // Optional: Print counter value using semihosting (for debugging)
            hprintln!("Counter: {}", current);
        }
        
        // Sleep to save power
        cortex_m::asm::wfi();
    }
}

// TIM2 interrupt handler
#[interrupt]
fn TIM2() {
    // Clear the interrupt flag
    free(|cs| {
        if let Some(tim2) = G_TIM2.borrow(cs).borrow_mut().deref_mut() {
            // This acknowledges the interrupt
            let _ = tim2.wait();
        }
        
        // Increment the counter
        let mut counter = G_COUNTER.borrow(cs).borrow_mut();
        *counter += 1;
    });
}
