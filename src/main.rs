#![no_std]
#![no_main]

mod ticker;
mod channel;
mod button;
mod button_interrupt;
mod led;
mod future;
mod executor;

use button::{ButtonTask, ButtonEvent};
use channel::Channel;
use ticker::Ticker;
use led::LedTask;
use future::OurFuture;

use stm32f0xx_hal::{pac, prelude::*};
use cortex_m_rt::entry;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

#[entry]
fn main() -> ! {
    // Initialize RTT
    rtt_init_print!();
    rprintln!("Starting program...");
    
    // Get access to the device peripherals
    let mut dp = pac::Peripherals::take().unwrap();
    
    rprintln!("Peripherals taken");
    
    // Configure the clock system more explicitly
    let mut rcc = dp.RCC.configure()
        .hsi48()                    // Use HSI48 (48 MHz internal oscillator)
        .enable_crs(dp.CRS)         // Enable Clock Recovery System
        .sysclk(48.mhz())           // Set system clock to 48 MHz
        .pclk(48.mhz())             // Set peripheral clock to 48 MHz
        .freeze(&mut dp.FLASH);
    
    rprintln!("Clocks configured");
    
    // Add a small delay to ensure clocks are stable
    cortex_m::asm::delay(1000);
    
    // Setup tick timer
    Ticker::init(dp.TIM2, &mut rcc);
    rprintln!("Ticker initialized");
    
    // Add another delay after timer setup
    cortex_m::asm::delay(1000);
    
    let channel: Channel<ButtonEvent> = Channel::new();

    // setup button    
    let gpioc = dp.GPIOC.split(&mut rcc);
    let button_pin = cortex_m::interrupt::free(|cs| {
        gpioc.pc13.into_floating_input(cs).downgrade()
    });
    rprintln!("Button pin configured");
    
    let mut button_task = ButtonTask::new(button_pin, &mut dp.SYSCFG, &mut dp.EXTI, channel.get_sender());
    rprintln!("Button task created");

    // setup led
    let gpioa = dp.GPIOA.split(&mut rcc);
    let user_led = cortex_m::interrupt::free(|cs| {
        gpioa.pa5.into_push_pull_output(cs).downgrade()
    });
    let mut led_task = LedTask::new(user_led, channel.get_receiver());
    
    rprintln!("LED task created");
    rprintln!("Starting executor...");

    let mut tasks: [&mut dyn OurFuture<Output = ()>; 2] = [
        &mut button_task,
        &mut led_task
    ];

    executor::run_tasks(&mut tasks);
}
