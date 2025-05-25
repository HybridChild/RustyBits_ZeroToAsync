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

pub struct TickTimer {
    end_time: TickInstant,
}

impl TickTimer {
    pub fn new(duration: TickDuration) -> Self {
        Self {
            end_time: Ticker::now() + duration,
        }
    }
    
    pub fn is_ready(&self) -> bool {
        Ticker::now() >= self.end_time
    }
}

static TICKER: Ticker = Ticker {
    tim2: Mutex::new(RefCell::new(None)),
    counter: Mutex::new(RefCell::new(0)),
};

pub struct Ticker {
    tim2: Mutex<RefCell<Option<Timer<TIM2>>>>,
    counter: Mutex<RefCell<u32>>,
}

impl Ticker {
    pub fn init(tim2: TIM2, rcc: &mut Rcc) {
        // Configure TIM2 for 1ms interrupts (1000 Hz)
        let mut timer = Timer::tim2(tim2, Hertz(1000), rcc);
        
        // Enable the timer interrupt
        timer.listen(Event::TimeOut);
        
        // Move timer into the global mutex
        free(|cs| {
            TICKER.tim2.borrow(cs).replace(Some(timer));
        });

        enable_tim2_interrupt();
    }
  
    pub fn now() -> TickInstant {
        let ticks = free(|cs| *TICKER.counter.borrow(cs).borrow());
        TickInstant::from_ticks(ticks)
    }
}

fn enable_tim2_interrupt() {
    // SAFETY: We enable this interrupt after setting up the TIM2 peripheral
    // correctly, and we ensure that the TIM2 interrupt handler is properly
    // defined to handle this interrupt. This operation is safe because 
    // we maintain exclusive control over the TIM2 peripheral.
    unsafe {
        NVIC::unmask(Interrupt::TIM2);
    }
}

// TIM2 interrupt handler
#[interrupt]
fn TIM2() {
    free(|cs| {
        if let Some(tim2) = TICKER.tim2.borrow(cs).borrow_mut().deref_mut() {
            // Clear the interrupt flag
            let _ = tim2.wait();
        }
        
        // Increment the counter
        let mut counter = TICKER.counter.borrow(cs).borrow_mut();
        *counter = counter.wrapping_add(1);
    });
}
