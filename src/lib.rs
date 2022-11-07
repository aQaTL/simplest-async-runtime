use log::{debug, error};
use std::collections::HashMap;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub mod sleep;

static WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);

static FUTURE_ID: AtomicU64 = AtomicU64::new(1);

pub struct Runtime {
    sender: Sender<RuntimeMsg>,
    receiver: Receiver<RuntimeMsg>,

    futures: HashMap<u64, (Box<dyn Future<Output = ()>>, Waker)>,
    main_future_id: u64,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<RuntimeMsg>();
        Runtime {
            sender,
            receiver,
            futures: HashMap::default(),
            main_future_id: 0,
        }
    }

    pub fn block_on<F: Future<Output = ()> + 'static>(&mut self, fut: F) {
        let future_id = FUTURE_ID.fetch_add(1, Ordering::Relaxed);
        let sender = self.sender.clone();
        let waker_ctx_ptr = Arc::into_raw(Arc::new(WakerContext { future_id, sender }));
        let waker =
            unsafe { Waker::from_raw(RawWaker::new(waker_ctx_ptr.cast::<()>(), &WAKER_VTABLE)) };

        self.futures.insert(future_id, (Box::new(fut), waker));
        self.main_future_id = future_id;
        self.sender.send(RuntimeMsg::Wake { future_id }).unwrap();

        for msg in self.receiver.iter() {
            match msg {
                RuntimeMsg::Wake { future_id } => {
                    let (fut, waker) = self
                        .futures
                        .get_mut(&future_id)
                        .expect("Tried to access future that doesn't exist");
                    let fut: Pin<&mut (dyn Future<Output = ()> + 'static)> =
                        unsafe { Pin::new_unchecked(fut.deref_mut()) };

                    let mut ctx = Context::from_waker(waker);

                    if let Poll::Ready(()) = fut.poll(&mut ctx) {
                        self.futures.remove(&future_id);
                        if future_id == self.main_future_id {
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum RuntimeMsg {
    Wake { future_id: u64 },
}

struct WakerContext {
    future_id: u64,
    sender: Sender<RuntimeMsg>,
}

unsafe fn clone_fn(waker_ctx: *const ()) -> RawWaker {
    debug!("clone_fn");

    let waker_ctx: Arc<WakerContext> = Arc::from_raw(waker_ctx.cast::<WakerContext>());
    let new_waker_ctx = Arc::clone(&waker_ctx);

    //Prevent deallocating pointer passed into this fn
    let _ = Arc::into_raw(waker_ctx);
    let data = Arc::into_raw(new_waker_ctx).cast::<()>();
    RawWaker::new(data, &WAKER_VTABLE)
}

unsafe fn wake_fn(waker_ctx: *const ()) {
    debug!("wake_fn");

    wake_by_ref_fn(waker_ctx);

    let _: Arc<WakerContext> = Arc::from_raw(waker_ctx.cast::<WakerContext>());
}

unsafe fn wake_by_ref_fn(waker_ctx: *const ()) {
    debug!("wake_by_ref_fn");

    let waker_ctx: Arc<WakerContext> = Arc::from_raw(waker_ctx.cast::<WakerContext>());

    let msg = RuntimeMsg::Wake {
        future_id: waker_ctx.future_id,
    };
    if let Err(err) = waker_ctx.sender.send(msg) {
        error!("Failed to send {msg:?}. {err:?}");
    }

    //Prevent deallocating pointer passed into this fn
    let _ = Arc::into_raw(waker_ctx);
}

unsafe fn drop_fn(waker_ctx: *const ()) {
    debug!("drop_fn");
    drop(Arc::from_raw((waker_ctx as *mut ()).cast::<WakerContext>()))
}
