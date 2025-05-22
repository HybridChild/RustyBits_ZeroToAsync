use embedded_hal::digital::PinState;
use core::sync::atomic::{AtomicUsize, Ordering};
use stm32f0xx_hal::{
    prelude::_embedded_hal_gpio_InputPin,
    pac::{interrupt, Interrupt, EXTI, SYSCFG},
    gpio::{Pin, Input, PullUp},
};
use rtt_target::rprintln;

use crate::{
    executor::wake_task,
    future::{OurFuture, Poll},
};

const MAX_CHANNELS_USED: usize = 1;
static NEXT_CHANNEL: AtomicUsize = AtomicUsize::new(0);

const INVALID_TASK_ID: usize = 0xFFFF_FFFF;
const DEFAULT_TASK: AtomicUsize = AtomicUsize::new(INVALID_TASK_ID);
static WAKE_TASKS: [AtomicUsize; MAX_CHANNELS_USED] = [DEFAULT_TASK; MAX_CHANNELS_USED];

pub struct InputChannel {
    pin: Pin<Input<PullUp>>,
    channel_id: usize,
    ready_state: PinState,
}

impl InputChannel {
    pub fn new(pin: Pin<Input<PullUp>>, syscfg: &mut SYSCFG, exti: &mut EXTI) -> Self {
        rprintln!("Initializing EXTI...");
        init(syscfg, exti);

        let channel_id = NEXT_CHANNEL.load(Ordering::Relaxed);
        NEXT_CHANNEL.store(channel_id + 1, Ordering::Relaxed);

        rprintln!("EXTI channel {} created", channel_id);

        Self {
            pin,
            channel_id,
            ready_state: PinState::Low,
        }
    }

    pub fn set_ready_state(&mut self, ready_state: PinState) {
        self.ready_state = ready_state;
    }
}

impl OurFuture for InputChannel {
    type Output = ();

    fn poll(&mut self, task_id: usize) -> Poll<Self::Output> {
        if self.ready_state == PinState::from(self.pin.is_high().unwrap()) {
            Poll::Ready(())
        } else {
            WAKE_TASKS[self.channel_id].store(task_id, Ordering::Relaxed);
            Poll::Pending
        }
    }
}

fn init(syscfg: &mut SYSCFG, exti: &mut EXTI) {
    rprintln!("Disabling EXTI interrupt...");
    // Disable the interrupt first to ensure clean setup
    cortex_m::peripheral::NVIC::mask(Interrupt::EXTI4_15);
    
    // Clear any pending interrupts first
    exti.pr.write(|w| w.pif13().set_bit());
    
    // Disable the EXTI line first
    exti.imr.modify(|_, w| w.mr13().clear_bit());
    
    rprintln!("Configuring SYSCFG for PC13...");
    // Configure PC13 as an external interrupt source
    // PC13 is connected to EXTI line 13
    syscfg.exticr4.modify(|_, w| w.exti13().pc13());
    
    // Clear both rising and falling edge triggers first
    exti.rtsr.modify(|_, w| w.tr13().clear_bit());
    exti.ftsr.modify(|_, w| w.tr13().clear_bit());
    
    rprintln!("Setting edge triggers...");
    // Set trigger on both edges to catch all button state changes
    exti.ftsr.modify(|_, w| w.tr13().set_bit());  // Falling edge (button press)
    exti.rtsr.modify(|_, w| w.tr13().set_bit());  // Rising edge (button release)
    
    // Clear any pending interrupts again
    exti.pr.write(|w| w.pif13().set_bit());
    
    // Enable the EXTI line
    exti.imr.modify(|_, w| w.mr13().set_bit());
    
    // Small delay to ensure hardware is ready
    cortex_m::asm::delay(100);
    
    rprintln!("Enabling NVIC interrupt...");
    enable_exti_interrupt();
    rprintln!("EXTI initialization complete");
}

fn enable_exti_interrupt() {
    // Enable the EXTI interrupt in NVIC
    unsafe {
        // Clear any pending interrupt in NVIC
        cortex_m::peripheral::NVIC::unpend(Interrupt::EXTI4_15);
        // Enable the interrupt
        cortex_m::peripheral::NVIC::unmask(Interrupt::EXTI4_15);
    }
}

// EXTI4_15 handles lines 4-15
#[interrupt]
fn EXTI4_15() {
    rprintln!("EXTI interrupt triggered!");
    
    // SAFETY: We're just reading and clearing interrupt flags
    let exti = unsafe { &*EXTI::ptr() };
    let line = 13;
    let idx = 0;

    let pr_value = exti.pr.read().bits();
    rprintln!("EXTI PR register: 0x{:08x}", pr_value);

    if pr_value & (1 << line) != 0 {
        rprintln!("Line 13 interrupt confirmed");
        
        // Clear the pending bit by writing 1 to it
        exti.pr.write(|w| unsafe { w.bits(1 << line) });

        // Swap in the INVALID_TASK_ID to prevent the task-ready queue from
        // getting filled up during debounce.
        let task_id = WAKE_TASKS[idx].load(Ordering::Relaxed);
        WAKE_TASKS[idx].store(INVALID_TASK_ID, Ordering::Relaxed);

        if task_id != INVALID_TASK_ID {
            rprintln!("Waking task {}", task_id);
            wake_task(task_id);
        } else {
            rprintln!("No valid task to wake");
        }
    } else {
        rprintln!("EXTI interrupt but not line 13");
    }
}
