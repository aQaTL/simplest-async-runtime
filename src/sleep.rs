use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}

pub struct Sleep {
    duration: Duration,
    start_time: Option<Instant>,
}

impl Sleep {
    pub fn new(duration: Duration) -> Self {
        Sleep {
            duration,
            start_time: None,
        }
    }

    fn expired(&self) -> bool {
        self.start_time
            .map(|start_time| start_time + self.duration <= Instant::now())
            .unwrap_or_default()
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.start_time {
            Some(_) => {
                if self.expired() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
            None => {
                self.start_time = Some(Instant::now());
                start_timer(self.duration, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

fn start_timer(duration: Duration, waker: Waker) {
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        waker.wake();
    });
}
