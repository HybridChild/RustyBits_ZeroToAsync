use embedded_hal::digital::PinState;
use core::sync::atomic::{AtomicUsize, Ordering};
use stm32f0xx_hal::{
    prelude::_embedded_hal_gpio_InputPin,
    pac::{interrupt, Interrupt, EXTI, SYSCFG},
    gpio::{Pin, Floating, Input},
};

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
    pin: Pin<Input<Floating>>,
    channel_id: usize,
    ready_state: PinState,
}

impl InputChannel {
    pub fn new(pin: Pin<Input<Floating>>, syscfg: &mut SYSCFG, exti: &mut EXTI) -> Self {
        init(syscfg, exti);

        let channel_id = NEXT_CHANNEL.load(Ordering::Relaxed);
        NEXT_CHANNEL.store(channel_id + 1, Ordering::Relaxed);

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
    // Configure PC13 as an external interrupt source
    // PC13 is connected to EXTI line 13
    syscfg.exticr4.modify(|_, w| w.exti13().pc13());
    
    // Configure EXTI line 13
    // Unmask the interrupt (enable it)
    exti.imr.modify(|_, w| w.mr13().set_bit());
    
    // Trigger on falling edge (button press)
    exti.ftsr.modify(|_, w| w.tr13().set_bit());
    
    // Clear any pending interrupts on this line
    exti.pr.write(|w| w.pif13().set_bit());

    enable_exti_interrupt();
}

fn enable_exti_interrupt() {
    // Enable the EXTI interrupt in NVIC
    unsafe {
        cortex_m::peripheral::NVIC::unmask(Interrupt::EXTI4_15);
    }
}

// EXTI4_15 handles lines 4-15
#[interrupt]
fn EXTI4_15() {
    // SAFETY: We're just reading and clearing interrupt flags
    let exti = unsafe { &*EXTI::ptr() };
    let line = 13;
    let idx = 0;

    if exti.pr.read().bits() & (1 << line) != 0 {
        // Clear the pending bit by writing 1 to it
        exti.pr.write(|w| unsafe { w.bits(1 << line) });

        // Swap in the INVALID_TASK_ID to prevent the task-ready queue from
        // getting filled up during debounce.
        let task_id = WAKE_TASKS[idx].load(Ordering::Relaxed);
        WAKE_TASKS[idx].store(INVALID_TASK_ID, Ordering::Relaxed);

        if task_id != INVALID_TASK_ID {
            wake_task(task_id);
        }
    }
}
