use crate::ticker::TickTimer;
use crate::channel::Sender;
use crate::future::OurFuture;

use fugit::MillisDuration;
use stm32f0xx_hal::{
    gpio::{gpioc::PC13, Floating, Input},
    prelude::_embedded_hal_gpio_InputPin
};

pub enum ButtonEvent {
    Pressed
}

enum ButtonState {
    WaitForPress,
    Debounce(TickTimer),
}

pub struct ButtonTask<'a> {
    pin: PC13<Input<Floating>>,
    state: ButtonState,
    debounce_duration: fugit::Duration<u32, 1, 1000>,
    sender: Sender<'a, ButtonEvent>,
}

impl<'a> ButtonTask<'a> {
    pub fn new(pin: PC13<Input<Floating>>, sender: Sender<'a, ButtonEvent>) -> Self {
        Self {
            pin,
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
                    if self.pin.is_low().unwrap() {
                        self.sender.send(ButtonEvent::Pressed);
                        self.state = ButtonState::Debounce(TickTimer::new(self.debounce_duration));
                    }
                    continue;
                }
                ButtonState::Debounce(ref timer) => {
                    if timer.is_ready() && self.pin.is_high().unwrap() {
                        self.state = ButtonState::WaitForPress;
                    }
                    continue;
                }
            }
            break;
        }

        return Poll::Pending;
    }
}
