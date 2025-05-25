#![no_std]
#![no_main]

mod button;
use button::{ButtonTask, ButtonEvent};

mod channel;
use channel::Channel;

mod ticker;
use ticker::Ticker;

mod led;
use led::LedTask;

use cortex_m_rt::entry;
use panic_halt as _;
use stm32f0xx_hal::{
    pac,
    prelude::*,
};

#[entry]
fn main() -> ! {
    // Get access to the device peripherals
    let mut dp = pac::Peripherals::take().unwrap();
    
    // Configure the clock system
    let mut rcc = dp.RCC.configure().freeze(&mut dp.FLASH);
    
    Ticker::init(dp.TIM2, &mut rcc);
    let channel: Channel<ButtonEvent> = Channel::new();

    // setup button    
    let gpioc = dp.GPIOC.split(&mut rcc);
    let button_pin = cortex_m::interrupt::free(|cs| gpioc.pc13.into_floating_input(cs));
    let mut button_task = ButtonTask::new(button_pin, channel.get_sender());

    // setup led
    let gpioa = dp.GPIOA.split(&mut rcc);
    let mut user_led = cortex_m::interrupt::free(|cs| gpioa.pa5.into_push_pull_output(cs));
    user_led.set_low().unwrap();
    let mut led_task = LedTask::new(user_led, channel.get_receiver());

    // Main loop
    loop {
        button_task.poll();
        led_task.poll();
    }
}
