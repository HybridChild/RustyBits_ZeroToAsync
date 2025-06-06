#![no_std]
#![no_main]

mod ticker;
mod channel;
mod button;
mod button_interrupt;
mod led;
mod executor;

use button::ButtonEvent;
use button_interrupt::InputChannel;
use channel::{Channel, Sender, Receiver};
use ticker::Ticker;
use led::LedThing;

use core::pin::pin;
use futures::{select_biased, FutureExt};
use fugit::MillisDurationU32;
use cortex_m_rt::entry;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};
use embedded_hal::digital::PinState;
use stm32f0xx_hal::{
    gpio::{Pin, Input, Output, PushPull, PullUp},
    pac::{self, EXTI, SYSCFG},
    prelude::*,
};

#[entry]
fn main() -> ! {
    // Initialize RTT
    rtt_init_print!();
    rprintln!("Starting program...");

    // Get access to the device peripherals
    let mut dp = pac::Peripherals::take().unwrap();

    rprintln!("Peripherals taken");

    // CRITICAL: Enable SYSCFG clock BEFORE calling RCC configure()
    // This is required for EXTI external interrupt pin mapping to work
    rprintln!("Enabling SYSCFG clock...");
    dp.RCC.apb2enr.modify(|_, w| w.syscfgen().set_bit());

    // Add delay to ensure clock propagation
    cortex_m::asm::delay(1000);

    // Verify SYSCFG clock is enabled and test register access
    let apb2enr = dp.RCC.apb2enr.read().bits();
    rprintln!("RCC APB2ENR: 0x{:08x} (SYSCFG clock enabled: {})", 
             apb2enr, if (apb2enr & 1) != 0 { "YES" } else { "NO" });

    // Now configure the main clock system (this consumes dp.RCC)
    let mut rcc = dp.RCC.configure()
        .hsi48()                    // Use HSI48 (48 MHz internal oscillator)
        .enable_crs(dp.CRS)         // Enable Clock Recovery System
        .sysclk(48.mhz())           // Set system clock to 48 MHz
        .pclk(48.mhz())             // Set peripheral clock to 48 MHz
        .freeze(&mut dp.FLASH);

    rprintln!("Clocks configured");

    // Add stabilization delays
    cortex_m::asm::delay(10000);

    // Setup GPIO with proper timing
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpioa = dp.GPIOA.split(&mut rcc);

    // Configure button pin and let it stabilize
    let button_pin = cortex_m::interrupt::free(|cs| {
        gpioc.pc13.into_pull_up_input(cs).downgrade()
    });
    rprintln!("Button pin configured (PC13: Pull-up Input)");

    // Long delay for pull-up to stabilize
    cortex_m::asm::delay(50000);

    // Configure LED pin
    let user_led = cortex_m::interrupt::free(|cs| {
        gpioa.pa5.into_push_pull_output(cs).downgrade()
    });
    rprintln!("LED pin configured (PA5: Push-Pull Output");

    // Setup on-demand tick timer
    Ticker::init(dp.TIM2, &mut rcc);
    rprintln!("On-demand ticker initialized");

    // Add delay after timer setup
    cortex_m::asm::delay(5000);

    // Create channel for button events
    let channel: Channel<ButtonEvent> = Channel::new();

    // Create button task (SYSCFG clock was enabled before RCC configure)
    rprintln!("Creating button task...");
    let exti_line_user_button = 13;
    let button_task = pin!(
        button_task(button_pin, exti_line_user_button, &mut dp.SYSCFG, &mut dp.EXTI, channel.get_sender())
    );
    rprintln!("Button task created");

    // Create LED task
    let led_task = pin!(led_task(user_led, channel.get_receiver()));
    rprintln!("LED task created");

    rprintln!("Starting executor...");
    executor::run_tasks(&mut [led_task, button_task]);
}

async fn led_task(
    led: Pin<Output<PushPull>>,
    mut receiver: Receiver<'_, ButtonEvent>
) {
    let mut blinker = LedThing::new(led);

    loop {
        blinker.toggle();

        select_biased! {
            button_event = receiver.receive().fuse() => {
                match button_event {
                    ButtonEvent::Pressed => {
                        blinker.update_blink_period();
                    }
                }
            }
            _ = ticker::delay(blinker.get_period()).fuse() => {}
        }
    }
}

async fn button_task(
    pin: Pin<Input<PullUp>>,
    exti_line: usize,
    syscfg: &mut SYSCFG,
    exti: &mut EXTI,
    sender: Sender<'_, ButtonEvent>
) {
    let mut input = InputChannel::new(pin, exti_line, syscfg, exti);

    loop {
        input.wait_for(PinState::Low).await;
        sender.send(ButtonEvent::Pressed);
        ticker::delay(MillisDurationU32::from_ticks(100)).await;
        input.wait_for(PinState::High).await;
    }
}
