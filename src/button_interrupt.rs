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
        let channel_id = NEXT_CHANNEL.load(Ordering::Relaxed);
        NEXT_CHANNEL.store(channel_id + 1, Ordering::Relaxed);

        let channel = Self {
            pin,
            channel_id,
            ready_state: PinState::Low,
        };

        // Allow pin to fully stabilize
        cortex_m::asm::delay(100000);
        
        // Initialize EXTI with proper SYSCFG configuration
        init_exti(syscfg, exti);
        
        channel
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

fn init_exti(syscfg: &mut SYSCFG, exti: &mut EXTI) {
    // Step 1: Disable everything first
    cortex_m::peripheral::NVIC::mask(Interrupt::EXTI4_15);
    exti.imr.modify(|_, w| w.mr13().clear_bit());
    exti.rtsr.modify(|_, w| w.tr13().clear_bit());
    exti.ftsr.modify(|_, w| w.tr13().clear_bit());
    exti.pr.write(|w| w.pif13().set_bit());
    cortex_m::asm::delay(1000);
    
    // Step 2: Map PC13 to EXTI13
    syscfg.exticr4.modify(|_, w| w.exti13().pc13());
    cortex_m::asm::delay(1000);
    
    // Step 3: Clear any pending interrupts after SYSCFG change
    exti.pr.write(|w| w.pif13().set_bit());
    cortex_m::asm::delay(1000);
    
    // Step 4: Enable FALLING and RISING edge detection
    exti.ftsr.modify(|_, w| w.tr13().set_bit());
    exti.rtsr.modify(|_, w| w.tr13().set_bit());

    cortex_m::asm::delay(1000);
    
    // Step 5: Clear any pending interrupts after trigger configuration
    exti.pr.write(|w| w.pif13().set_bit());
    cortex_m::asm::delay(1000);
    
    // Step 6: Enable interrupt mask
    exti.imr.modify(|_, w| w.mr13().set_bit());
    cortex_m::asm::delay(100);
    
    // Step 7: Enable NVIC interrupt
    unsafe {
        cortex_m::peripheral::NVIC::unpend(Interrupt::EXTI4_15);
        cortex_m::peripheral::NVIC::unmask(Interrupt::EXTI4_15);
    }

    rprintln!("EXTI configured (Trigger interrupt on PC13 falling and rising edge)");
}

// EXTI4_15 interrupt handler
#[interrupt]
fn EXTI4_15() {
    let exti = unsafe { &*EXTI::ptr() };
    let idx = 0;

    if exti.pr.read().pif13().bit() {
        rprintln!("Button interrupt detected");
        
        // Clear the pending bit
        exti.pr.write(|w| w.pif13().set_bit());

        // Wake the corresponding task
        let task_id = WAKE_TASKS[idx].load(Ordering::Relaxed);

        if task_id != INVALID_TASK_ID {
            rprintln!("Waking task {}", task_id);
            wake_task(task_id);
        }
    }
}
