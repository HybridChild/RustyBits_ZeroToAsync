use core::cell::RefCell;
use core::ops::DerefMut;
use cortex_m::interrupt::{free, Mutex};
use cortex_m::peripheral::NVIC;
use panic_halt as _;
use stm32f0xx_hal::{
    pac::{interrupt, Interrupt, TIM2},
    rcc::Rcc,
    prelude::*,
    time::Hertz,
    timers::{Event, Timer},
};


// Global variable to share the timer interrupt data
static G_TIM2: Mutex<RefCell<Option<Timer<TIM2>>>> = Mutex::new(RefCell::new(None));
// Counter to track interrupt occurrences
pub static G_COUNTER: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));


pub fn config_timer(tim2: TIM2, rcc: &mut Rcc) {
    // Configure TIM2 for 1ms interrupts (1000 Hz)
    let mut timer = Timer::tim2(tim2, Hertz(1000), rcc);
    
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
