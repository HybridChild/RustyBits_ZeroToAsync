use core::{cell::RefCell, ops::DerefMut};
use fugit::{TimerDuration, TimerInstant};
use heapless::{binary_heap::Min, BinaryHeap};

use cortex_m::{
    interrupt::{free, Mutex},
    peripheral::NVIC,
};

use stm32f0xx_hal::{
  pac::{interrupt, Interrupt, TIM2},
  rcc::Rcc,
  prelude::*,
  time::Hertz,
  timers::{Event, Timer},
};

use crate::{
    future::{OurFuture, Poll},
    executor::wake_task,
};

// Define our time types with millisecond precision
pub type TickDuration = TimerDuration<u32, 1000>; // 1ms precision (1000 Hz)
pub type TickInstant = TimerInstant<u32, 1000>;   // 1ms precision (1000 Hz)

// Static variables
const MAX_DEADLINES: usize = 8;
static WAKE_DEADLINES: Mutex<RefCell<BinaryHeap<(u32, usize), Min, MAX_DEADLINES>>> =
    Mutex::new(RefCell::new(BinaryHeap::new()));

static TICKER: Ticker = Ticker {
    tim2: Mutex::new(RefCell::new(None)),
    counter: Mutex::new(RefCell::new(0)),
};

// TickTimer struct
enum TimerState {
    Init,
    Wait,
}

pub struct TickTimer {
    end_time: TickInstant,
    state: TimerState,
}

impl TickTimer {
    pub fn new(duration: TickDuration) -> Self {
        Self {
            end_time: Ticker::now() + duration,
            state: TimerState::Init,
        }
    }

    /// Register this timer's deadline in the wake queue
    fn register(&self, task_id: usize) {
        let new_deadline = self.end_time.ticks();

        free(|cs| {
            let mut deadlines = WAKE_DEADLINES.borrow(cs).borrow_mut();
            
            if deadlines.push((new_deadline, task_id)).is_err() {
                panic!("Deadline dropped for task {}!", task_id);
            }
        });
    }
}

impl OurFuture for TickTimer {
    type Output = ();
    
    fn poll(&mut self, task_id: usize) -> Poll<Self::Output> {
        match self.state {
            TimerState::Init => {
                self.register(task_id);
                self.state = TimerState::Wait;
                Poll::Pending
            }
            TimerState::Wait => {
                if Ticker::now() >= self.end_time {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

// Ticker struct
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
        // Clear the interrupt flag
        if let Some(tim2) = TICKER.tim2.borrow(cs).borrow_mut().deref_mut() {
            let _ = tim2.wait();
        }
        
        // Increment the counter
        let mut counter = TICKER.counter.borrow(cs).borrow_mut();
        *counter = counter.wrapping_add(1);
        
        // Check for expired deadlines and wake tasks
        let current_time = *counter;
        let mut deadlines = WAKE_DEADLINES.borrow(cs).borrow_mut();
        
        while let Some((deadline, task_id)) = deadlines.peek() {
            if current_time >= *deadline {
                wake_task(*task_id);
                deadlines.pop();
            } else {
                break;
            }
        }
    });
}
