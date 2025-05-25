use cortex_m::asm;
use heapless::mpmc::Q4;
use rtt_target::rprintln;

use crate::future::OurFuture;

static TASK_IS_READY: Q4<usize> = Q4::new();

pub fn wake_task(task_id: usize) {
    rprintln!("Waking task {}", task_id);

    if TASK_IS_READY.enqueue(task_id).is_err() {
        panic!("Task queue full: can't add task {}", task_id);
    }
}

pub fn run_tasks(tasks: &mut [&mut dyn OurFuture<Output = ()>]) -> ! {
    // Initially wake all tasks to let them register their first deadlines
    for task_id in 0..tasks.len() {
        TASK_IS_READY.enqueue(task_id).ok();
    }

    loop {
        while let Some(task_id) = TASK_IS_READY.dequeue() {
            if task_id >= tasks.len() {
                rprintln!("Bad task id {}!", task_id);
                continue;
            }

            rprintln!("Running task {}", task_id);
            tasks[task_id].poll(task_id);
        }

        // Enter sleep mode - processor will wake on any interrupt
        // (timer compare, button interrupt, etc.)
        rprintln!("Entering sleep mode...");
        asm::wfi();
        rprintln!("Woke from sleep");
    }
}
