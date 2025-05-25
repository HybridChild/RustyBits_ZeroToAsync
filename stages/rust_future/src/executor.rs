use cortex_m::asm;
use heapless::mpmc::Q4;
use rtt_target::rprintln;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, RawWaker, RawWakerVTable, Waker},
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait ExtWaker {
    fn task_id(&self) -> usize;
}

impl ExtWaker for Waker {
    fn task_id(&self) -> usize {
        //return (self.as_raw().data() as usize);

        for task_id in 0..NUM_TASKS.load(Ordering::Relaxed) {
            if get_waker(task_id).will_wake(self) {
                return task_id;
            }
        }
        panic!("Unknown waker/executor!");
    }
}

fn get_waker(task_id: usize) -> Waker {
    // SAFETY:
    // Data argument interpreted as an integer, not dereferenced
    unsafe {
        Waker::from_raw(RawWaker::new(task_id as *const (), &VTABLE))
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

unsafe fn clone(p: *const ()) -> RawWaker {
    RawWaker::new(p, &VTABLE)
}

unsafe fn drop(_p: *const ()) {}

unsafe fn wake(p: *const ()) {
    wake_task(p as usize);
}
unsafe fn wake_by_ref(p: *const ()) {
    wake_task(p as usize);
}

static NUM_TASKS: AtomicUsize = AtomicUsize::new(0);
static TASK_IS_READY: Q4<usize> = Q4::new();

pub fn wake_task(task_id: usize) {
    rprintln!("Waking task {}", task_id);

    if TASK_IS_READY.enqueue(task_id).is_err() {
        panic!("Task queue full: can't add task {}", task_id);
    }
}

pub fn run_tasks(tasks: &mut [Pin<&mut dyn Future<Output = ()>>]) -> ! {
    NUM_TASKS.store(tasks.len(), Ordering::Relaxed);

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
            let _ = tasks[task_id]
                .as_mut()
                .poll(&mut Context::from_waker(&get_waker(task_id)));
        }

        // Enter sleep mode - processor will wake on any interrupt
        // (timer compare, button interrupt, etc.)
        rprintln!("Entering sleep mode...");
        asm::wfi();
        rprintln!("Woke from sleep");
    }
}
