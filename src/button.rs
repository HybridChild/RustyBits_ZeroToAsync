use embedded_hal::digital::PinState;
use fugit::MillisDuration;
use stm32f0xx_hal::{
    gpio::{Pin, Input, PullUp}, pac::{EXTI, SYSCFG}
};

use crate::{
    ticker::{TickTimer, TickDuration},
    channel::Sender,
    future::{OurFuture, Poll},
    button_interrupt::InputChannel,
};

pub enum ButtonEvent {
    Pressed
}

enum ButtonState {
    WaitForPress,
    WaitForRelease,
    Debounce(TickTimer),
}

pub struct ButtonTask<'a> {
    input: InputChannel,
    state: ButtonState,
    debounce_duration: TickDuration,
    sender: Sender<'a, ButtonEvent>,
}

impl<'a> ButtonTask<'a> {
    pub fn new(pin: Pin<Input<PullUp>>, exti_line: usize, syscfg: &mut SYSCFG, exti: &mut EXTI, sender: Sender<'a, ButtonEvent>) -> Self {
        Self {
            input: InputChannel::new(pin, exti_line, syscfg, exti),
            state: ButtonState::WaitForPress,
            debounce_duration: MillisDuration::<u32>::from_ticks(100),
            sender,
        }
    }
}

impl OurFuture for ButtonTask<'_> {
    type Output = ();

    fn poll(&mut self, task_id: usize) -> Poll<Self::Output> {
        loop {
            match self.state {
                ButtonState::WaitForPress => {
                    self.input.set_ready_state(PinState::Low);

                    if let Poll::Ready(_) = self.input.poll(task_id) {
                        self.sender.send(ButtonEvent::Pressed);
                        self.state = ButtonState::Debounce(TickTimer::new(self.debounce_duration));
                        continue;
                    }
                }
                ButtonState::Debounce(ref mut timer) => {
                    if let Poll::Ready(_) = timer.poll(task_id) {
                        self.state = ButtonState::WaitForRelease;
                        continue;
                    }
                }
                ButtonState::WaitForRelease => {
                    self.input.set_ready_state(PinState::High);

                    if let Poll::Ready(_) = self.input.poll(task_id) {
                        self.state = ButtonState::WaitForPress;
                        continue;
                    }
                }
            }

            break;
        }

        Poll::Pending
    }
}
