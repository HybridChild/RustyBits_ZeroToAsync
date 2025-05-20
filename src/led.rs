use stm32f0xx_hal::{
    gpio::{gpioa::PA5, Output, PushPull},
    prelude::{_embedded_hal_gpio_OutputPin, _embedded_hal_gpio_ToggleableOutputPin},
};
use fugit::MillisDuration;
use crate::ticker::TickTimer;
use crate::channel::Receiver;
use crate::button::ButtonEvent;
use crate::future::OurFuture;

enum LedState {
    Toggle,
    Wait(TickTimer),
}

pub struct LedTask<'a> {
    led: PA5<Output<PushPull>>,
    blink_period: fugit::Duration<u32, 1, 1000>,
    state: LedState,
    receiver: Receiver<'a, ButtonEvent>,
}

impl<'a> LedTask<'a> {
    pub fn new(led: PA5<Output<PushPull>>, receiver: Receiver<'a, ButtonEvent>) -> Self {
        Self {
            led,
            blink_period: MillisDuration::<u32>::from_ticks(500),
            state: LedState::Toggle,
            receiver,
        }
    }

    fn update_blink_period(&mut self) {
        let current_period = self.blink_period.to_millis();

        if current_period < 100 {
            self.blink_period = MillisDuration::<u32>::from_ticks(500);
        } else {
            self.blink_period = self.blink_period - MillisDuration::<u32>::from_ticks(current_period >> 1);
        }
    }
}

impl OurFuture for LedTask<'_> {
    type Output = ();

    fn poll(&mut self, task_id: usize) -> Poll<Self::Output> {
        loop {
            match self.receiver.receive() {
                None => {},
                Some(event) => {
                    match event {
                        ButtonEvent::Pressed => {
                            self.led.set_low().unwrap();
                            self.update_blink_period();
                            self.state = LedState::Toggle;
                            continue;
                        }
                    }
                }
            }
    
            match self.state {
                LedState::Toggle => {
                    self.led.toggle().unwrap();
                    self.state = LedState::Wait(TickTimer::new(self.blink_period));
                    continue;
                }
                LedState::Wait(ref timer) => {
                    if timer.is_ready() {
                        self.state = LedState::Toggle;
                    }
                    continue;
                }
            }
            break;
        }
        
        return Poll::Pending;
    }
}
