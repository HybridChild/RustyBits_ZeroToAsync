use fugit::MillisDurationU32;
use stm32f0xx_hal::{
    gpio::{Pin, Output, PushPull},
    prelude::_embedded_hal_gpio_ToggleableOutputPin,
};

use crate::ticker::TickDuration;

pub struct LedThing {
    led: Pin<Output<PushPull>>,
    blink_period: TickDuration,
}

impl LedThing {
    pub fn new(led: Pin<Output<PushPull>>) -> Self {
        Self {
            led,
            blink_period: MillisDurationU32::from_ticks(500),
        }
    }

    pub fn update_blink_period(&mut self) {
        let current_period = self.blink_period.to_millis();

        if current_period < 100 {
            self.blink_period = MillisDurationU32::from_ticks(500);
        } else {
            self.blink_period -= MillisDurationU32::from_ticks(current_period >> 1);
        }
    }

    pub fn get_period(&self) -> TickDuration {
        self.blink_period
    }

    pub fn toggle(&mut self) {
        self.led.toggle().unwrap();
    }
}
