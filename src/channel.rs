use core::{
    cell::{Cell, RefCell},
    future::poll_fn,
    task::{Context, Poll, Waker},
};

pub struct Channel<T> {
    item: Cell<Option<T>>,
    waker: RefCell<Option<Waker>>,
}

impl<T> Channel<T> {
    pub fn new() -> Self {
        Self {
            item: Cell::new(None),
            waker: RefCell::new(None),
        }
    }

    pub fn get_sender(&self) -> Sender<T> {
        Sender { channel: self }
    }

    pub fn get_receiver(&self) -> Receiver<T> {
        Receiver {
            channel: self,
            state: ReceiverState::Init,
        }
    }

    fn send(&self, item: T) {
        self.item.replace(Some(item));

        if let Some(waker) = self.waker.borrow().as_ref() {
            waker.wake_by_ref();
        }
    }

    fn receive(&self) -> Option<T> {
        self.item.take()
    }

    fn register(&self, waker: Waker) {
        self.waker.replace(Some(waker));
    }
}

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
}

impl<T> Sender<'_, T> {
    pub fn send(&self, item: T) {
        self.channel.send(item);
    }
}

enum ReceiverState {
    Init,
    Wait,
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
    state: ReceiverState,
}

impl<T> Receiver<'_, T> {
    pub async fn receive(&mut self) -> T {
        poll_fn(|cx: &mut Context| {
            match self.state {
                ReceiverState::Init => {
                    self.channel.register(cx.waker().clone());
                    self.state = ReceiverState::Wait;
                    Poll::Pending
                }
                ReceiverState::Wait => {
                    match self.channel.receive() {
                        Some(item) => Poll::Ready(item),
                        None => Poll::Pending,
                    }
                }

            }
        }).await
    }
}
