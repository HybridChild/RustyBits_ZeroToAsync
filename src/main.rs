#![no_std]
#![no_main]

mod ticker;
mod channel;
mod button;
mod led;
mod future;
mod executor;

use button::{ButtonTask, ButtonEvent};
use channel::Channel;
use ticker::Ticker;
use led::LedTask;
use future::OurFuture;

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
    let button_pin = button_pin.downgrade();
    let mut button_task = ButtonTask::new(button_pin, channel.get_sender());

    // setup led
    let gpioa = dp.GPIOA.split(&mut rcc);
    let mut user_led = cortex_m::interrupt::free(|cs| gpioa.pa5.into_push_pull_output(cs));
    let mut user_led = user_led.downgrade();
    user_led.set_low().unwrap();
    let mut led_task = LedTask::new(user_led, channel.get_receiver());

    let mut tasks: [&mut dyn OurFuture<Output = ()>; 2] = [
        &mut button_task,
        &mut led_task
    ];

    executor::run_tasks(&mut tasks);
}
