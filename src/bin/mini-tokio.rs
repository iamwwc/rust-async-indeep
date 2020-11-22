use std::{time::Duration, cell::RefCell, sync::Mutex, task::Waker, time::Instant};
use std::thread;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::task::{self, ArcWake};
use crossbeam::channel;
fn main() {
    let mini_tokio = MiniTokio::new();
    mini_tokio.spawn(async {
        spawn(async {
            delay(Duration::from_millis(100)).await;
            println!("world");
        });
        spawn(async {
            println!("hello");
        });
        delay(Duration::from_millis(200)).await;
        std::process::exit(0)
    });
    mini_tokio.run();
}
pub fn spawn<F>(future:F) where F: Future<Output = ()> + Send + 'static,{
    CURRENT.with(|cell| {
        let borrow = cell.borrow();
        let sender = borrow.as_ref().unwrap();
        Task::spawn(future, sender);
    })
}
struct MiniTokio {
    scheduled: channel::Receiver<Arc<Task>>,
    sender: channel::Sender<Arc<Task>>
}

// type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

struct Task {
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    executor: channel::Sender<Arc<Task>>,
}
thread_local! {
    static CURRENT: RefCell<Option<channel::Sender<Arc<Task>>>> = RefCell::new(None);
}
impl Task {
    fn poll(self: Arc<Self>) {
        let waker = task::waker(self.clone());
        let mut cx = Context::from_waker(&waker);
        let mut future = self.future.try_lock().unwrap();
        let _ = future.as_mut().poll(&mut cx);
    }
    fn spawn<F>(future: F, sender: &channel::Sender<Arc<Task>>) where F: Future<Output = ()> + Send + 'static {
        let task = Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
            executor: sender.clone(),
        });
        let _ = sender.send(task);
    }
}

impl MiniTokio {
    fn new () -> MiniTokio {
        let (sender, scheduled) = channel::unbounded();
        MiniTokio {
            scheduled,sender,
        }
    }

    fn spawn<F>(&self, future: F) where F: Future<Output = ()> + Send + 'static {
        Task::spawn(future, &self.sender);
    }

    fn run(&self){
        CURRENT.with(|cell| {
            *cell.borrow_mut() = Some(self.sender.clone());
        });
        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let _ = arc_self.executor.send(arc_self.clone());
    }
}

async fn delay(dur: Duration) {
    struct Delay {
        when: Instant,
        waker: Option<Arc<Mutex<Waker>>>,
    }

    impl Future for Delay {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if let Some(waker) = &self.waker {
                let mut waker = waker.lock().unwrap();
                if !waker.will_wake(cx.waker()) {
                    *waker = cx.waker().clone();
                }
            }else {
                let when = self.when;
                let waker = Arc::new(Mutex::new(cx.waker().clone()));
                self.waker = Some(waker.clone());
                thread::spawn(move || {
                    let now = Instant::now();
                    if now < when {
                        thread::sleep(when - now);
                    }
                    let waker = waker.lock().unwrap();
                    waker.wake_by_ref();
                });
            }
            if Instant::now() >= self.when {
                Poll::Ready(())
            }else {
                Poll::Pending
            }
        }
    }
    let future = Delay{
        when: Instant::now() + dur,
        waker: None,
    };
    future.await;
}