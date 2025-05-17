use fugit::{TimerDuration, TimerInstant};
use core::cell::RefCell;
use core::ops::DerefMut;
use cortex_m::interrupt::{free, Mutex};
use cortex_m::peripheral::NVIC;

use stm32f0xx_hal::{
  pac::{interrupt, Interrupt, TIM2},
  rcc::Rcc,
  prelude::*,
  time::Hertz,
  timers::{Event, Timer},
};

// Define our time types with millisecond precision
pub type TickDuration = TimerDuration<u32, 1000>; // 1ms precision (1000 Hz)
pub type TickInstant = TimerInstant<u32, 1000>;   // 1ms precision (1000 Hz)

// Global variable to share the timer interrupt data
static G_TIM2: Mutex<RefCell<Option<Timer<TIM2>>>> = Mutex::new(RefCell::new(None));
// Counter to track interrupt occurrences
pub static G_COUNTER: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));


pub struct TickTimer<'a> {
    running: bool,
    duration: TickDuration,
    end_time: TickInstant,
    ticker: &'a Ticker,
}

impl<'a> TickTimer<'a> {
    pub fn new(duration: TickDuration, ticker: &'a Ticker) -> Self {
        Self {
            running: false,
            duration: duration,
            end_time: TickInstant::from_ticks(0),
            ticker,
        }
    }

    pub fn start(&mut self) {
        self.running = true;
        self.reset();
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_ready(&self) -> bool {
        self.running && self.ticker.now() >= self.end_time
    }

    fn reset(&mut self) {
        self.end_time = self.ticker.now() + self.duration;
    }
}


pub struct Ticker {}

impl Ticker {
    pub fn new(tim2: TIM2, rcc: &mut Rcc) -> Self {
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

        Self {}
    }
  
    pub fn now(&self) -> TickInstant {
        // Use our counter as the source of time
        let ticks = free(|cs| *G_COUNTER.borrow(cs).borrow());
        TickInstant::from_ticks(ticks)
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
        *counter = counter.wrapping_add(1);
    });
}