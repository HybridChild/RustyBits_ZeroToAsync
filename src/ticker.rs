use core::cell::RefCell;
use fugit::{TimerDuration, TimerInstant};
use heapless::{binary_heap::Min, BinaryHeap};

use cortex_m::{
    interrupt::{free, Mutex, CriticalSection},
    peripheral::NVIC,
};

use stm32f0xx_hal::{
    pac::{interrupt, Interrupt, TIM2},
    rcc::Rcc,
    timers::{Timer},
};

use crate::{
    future::{OurFuture, Poll},
    executor::wake_task,
};

// Define our time types with millisecond precision
pub type TickDuration = TimerDuration<u32, 1000>; // 1ms precision (1000 Hz)
pub type TickInstant = TimerInstant<u32, 1000>;   // 1ms precision (1000 Hz)

// Constants
const MAX_DEADLINES: usize = 8;
const TIMER_MAX_COUNT: u32 = 0xFFFFFFFF; // TIM2 is 32-bit
const HEARTBEAT_INTERVAL_MS: u32 = 24 * 60 * 60 * 1000; // 24 hours in ms
const HEARTBEAT_TASK_ID: usize = 0xFFFF_FFFF; // Special task ID for heartbeat

// Static variables
static WAKE_DEADLINES: Mutex<RefCell<BinaryHeap<(u32, usize), Min, MAX_DEADLINES>>> =
    Mutex::new(RefCell::new(BinaryHeap::new()));

static TICKER: Ticker = Ticker {
    tim2: Mutex::new(RefCell::new(None)),
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

    /// Register this timer's deadline and set compare interrupt if needed
    fn register(&self, task_id: usize) {
        let global_deadline = self.end_time.ticks();

        free(|cs| {
            let mut deadlines = WAKE_DEADLINES.borrow(cs).borrow_mut();

            if deadlines.push((global_deadline, task_id)).is_err() {
                panic!("Deadline dropped for task {}!", task_id);
            }

            drop(deadlines);  // Release borrow before calling other functions

            // This will either set compare for future deadline OR wake immediately if expired
            update_compare_for_earliest_deadline(cs);
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
}

impl Ticker {
    pub fn init(_tim2: TIM2, rcc: &mut Rcc) {
        // Manual timer configuration for 1ms resolution

        // Enable TIM2 clock manually using the RCC peripheral
        unsafe {
            let rcc_reg = &*stm32f0xx_hal::pac::RCC::ptr();
            rcc_reg.apb1enr.modify(|_, w| w.tim2en().set_bit());
            rcc_reg.apb1rstr.modify(|_, w| w.tim2rst().set_bit());
            rcc_reg.apb1rstr.modify(|_, w| w.tim2rst().clear_bit());
        }

        unsafe {
            let tim2_reg = &*TIM2::ptr();

            // Stop timer
            tim2_reg.cr1.modify(|_, w| w.cen().clear_bit());

            // Calculate prescaler for 1ms resolution
            // Timer clock = PCLK (or PCLK*2 if PCLK is prescaled from HCLK)
            let timer_clock = if rcc.clocks.hclk().0 == rcc.clocks.pclk().0 {
                rcc.clocks.pclk().0  // PCLK not prescaled
            } else {
                rcc.clocks.pclk().0 * 2  // PCLK prescaled, so timer gets 2x
            };

            // We want 1000 Hz (1ms per tick)
            // PSC = (timer_clock / desired_frequency) - 1
            let prescaler = (timer_clock / 1000) - 1;
            tim2_reg.psc.write(|w| w.psc().bits(prescaler as u16));

            // Set ARR to maximum for free-running
            tim2_reg.arr.write(|w| w.bits(TIMER_MAX_COUNT));

            // Reset counter
            tim2_reg.cnt.reset();

            // Force an update event to load the prescaler value
            // This is crucial - the prescaler is buffered and needs an update event
            tim2_reg.egr.write(|w| w.ug().set_bit());

            // Clear the update flag that was just set by the update event
            tim2_reg.sr.modify(|_, w| w.uif().clear_bit());

            // Configure for basic timer operation
            tim2_reg.cr1.write(|w| w
                .cen().clear_bit()    // Counter disabled for now
                .udis().clear_bit()   // Update events enabled
                .urs().clear_bit()    // Update request source
                .opm().clear_bit()    // One-pulse mode disabled (continuous)
                .dir().clear_bit()    // Count up
                .cms().bits(0)        // Edge-aligned mode
                .arpe().clear_bit()   // Auto-reload preload disabled
            );

            // Disable all interrupts initially
            tim2_reg.dier.write(|w| w.cc1ie().clear_bit().uie().clear_bit());

            // Clear all interrupt flags
            tim2_reg.sr.write(|w| w.cc1if().clear_bit().uif().clear_bit());

            // Start timer
            tim2_reg.cr1.modify(|_, w| w.cen().set_bit());
        }

        // Store a dummy timer (we're managing everything manually)
        free(|cs| {
            TICKER.tim2.borrow(cs).replace(None);
            // Queue starts empty, so update_compare_for_earliest_deadline will add heartbeat
            update_compare_for_earliest_deadline(cs);
        });

        enable_tim2_interrupt();
    }

    pub fn now() -> TickInstant {
        TickInstant::from_ticks(read_timer_counter())
    }
}

/// Read current timer counter value
fn read_timer_counter() -> u32 {
    unsafe {
        let tim2_reg = &*TIM2::ptr();
        tim2_reg.cnt.read().bits()
    }
}

/// Set compare register for a specific global deadline
fn set_compare_for_deadline(cs: &CriticalSection, global_deadline: u32) {
    let current_global_time = read_timer_counter();

    if global_deadline <= current_global_time {
        // Deadline already passed - wake expired tasks immediately
        wake_expired_deadlines_now(cs, current_global_time);
        return;
    }

    let time_until_deadline = global_deadline - current_global_time;
    let remaining_timer_ticks = TIMER_MAX_COUNT - current_global_time;

    // Check if deadline fits within current timer cycle
    if time_until_deadline <= remaining_timer_ticks {
        let target_timer_count = current_global_time + time_until_deadline;

        unsafe {
            let tim2_reg = &*TIM2::ptr();
            tim2_reg.ccr1.write(|w| w.bits(target_timer_count));
            tim2_reg.dier.modify(|_, w| w.cc1ie().set_bit()); // Compare 1 interrupt enable
        }
    } else {
        // Deadline beyond current cycle - disable compare, will be handled after wraparound
        disable_compare_interrupt(cs);
    }
}

/// Disable compare interrupt
fn disable_compare_interrupt(_cs: &CriticalSection) {
    unsafe {
        let tim2_reg = &*TIM2::ptr();
        tim2_reg.dier.modify(|_, w| w.cc1ie().clear_bit());
    }
}

/// Wake all expired deadlines immediately
fn wake_expired_deadlines_now(cs: &CriticalSection, current_time: u32) {
    let mut deadlines = WAKE_DEADLINES.borrow(cs).borrow_mut();

    // Wake all expired deadlines
    while let Some((deadline, task_id)) = deadlines.peek() {
        if *deadline <= current_time {
            if *task_id != HEARTBEAT_TASK_ID {
                wake_task(*task_id);  // Only wake real tasks, not heartbeat
            }
            deadlines.pop();
        } else {
            break;
        }
    }

    drop(deadlines);  // Release borrow

    // After removing expired deadlines, set compare for next earliest (if any)
    update_compare_for_earliest_deadline(cs);
}

/// Update compare register for the earliest deadline in the heap
fn update_compare_for_earliest_deadline(cs: &CriticalSection) {
    let deadlines = WAKE_DEADLINES.borrow(cs).borrow();

    if let Some((earliest_deadline, _)) = deadlines.peek() {
        let earliest = *earliest_deadline;
        drop(deadlines);  // Release borrow before calling set_compare
        set_compare_for_deadline(cs, earliest);
    } else {
        // Queue is empty - add heartbeat and set compare for it
        drop(deadlines);  // Release borrow

        let current_time = read_timer_counter();
        let next_heartbeat = current_time.wrapping_add(HEARTBEAT_INTERVAL_MS);

        let mut deadlines = WAKE_DEADLINES.borrow(cs).borrow_mut();
        if deadlines.push((next_heartbeat, HEARTBEAT_TASK_ID)).is_ok() {
            drop(deadlines);
            set_compare_for_deadline(cs, next_heartbeat);
        } else {
            // Should never happen since queue was empty
            drop(deadlines);
            disable_compare_interrupt(cs);
        }
    }
}

/// Wake all tasks with deadlines <= current_time
fn wake_tasks_with_deadline(cs: &CriticalSection, current_time: u32) {
    let mut deadlines = WAKE_DEADLINES.borrow(cs).borrow_mut();

    while let Some((deadline, task_id)) = deadlines.peek() {
        if *deadline <= current_time {
            if *task_id != HEARTBEAT_TASK_ID {
                wake_task(*task_id);  // Only wake real tasks, not heartbeat
            }
            deadlines.pop();
        } else {
            break;
        }
    }
}

fn enable_tim2_interrupt() {
    // SAFETY: We enable this interrupt after setting up the TIM2 peripheral
    // correctly, and we ensure that the TIM2 interrupt handler is properly
    // defined to handle this interrupt. This operation is safe because 
    // we maintain exclusive control over the TIM2 peripheral.
    unsafe {
        NVIC::unpend(Interrupt::TIM2);
        NVIC::unmask(Interrupt::TIM2);
    }
}

// TIM2 interrupt handler
#[interrupt]
fn TIM2() {
    free(|cs| {
        let tim2_reg = unsafe { &*TIM2::ptr() };

        // Handle compare interrupt (deadline reached)
        if tim2_reg.sr.read().cc1if().bit_is_set() {
            tim2_reg.sr.modify(|_, w| w.cc1if().clear_bit());

            // Calculate current time and wake expired tasks
            let current_time = read_timer_counter();
            wake_tasks_with_deadline(cs, current_time);

            // Set compare for next earliest deadline
            update_compare_for_earliest_deadline(cs);
        }
    });
}
