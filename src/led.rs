use stm32f0xx_hal::{
    gpio::{gpioa::PA5, Output, PushPull},
    prelude::_embedded_hal_gpio_ToggleableOutputPin,
};
use fugit::MillisDuration;
use crate::ticker::{Ticker, TickTimer};

enum LedState<'a> {
    Toggle,
    Wait(TickTimer<'a>),
}

pub struct LedTask<'a> {
    led: PA5<Output<PushPull>>,
    ticker: &'a Ticker,
    blink_period: u32,
    state: LedState<'a>,
}

impl<'a> LedTask<'a> {
    pub fn new(led: PA5<Output<PushPull>>, ticker: &'a Ticker) -> Self {
        Self {
            led,
            ticker,
            blink_period: 1000,
            state: LedState::Toggle,
        }
    }

    pub fn poll(&mut self) {
        match self.state {
            LedState::Toggle => {
                self.led.toggle().unwrap();
                let duration = MillisDuration::<u32>::from_ticks(self.blink_period);
                self.state = LedState::Wait(TickTimer::new(duration, &self.ticker));
            }
            LedState::Wait(ref timer) => {
                if timer.is_ready() {
                    self.state = LedState::Toggle;
                }
            }
        }
    }
}
