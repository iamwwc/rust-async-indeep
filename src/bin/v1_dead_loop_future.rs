use std::{future::Future, pin::Pin, task::Context, time::Duration, task::Poll, time::Instant};

use tokio::net::TcpStream;

async fn my_async_fn () {
    println!("hello from async");
    let _socket = TcpStream::connect("127.0.0.1:3000").await.unwrap();
    println!("async TCP operation complete");
}
#[tokio::main]
async fn main() {
    let when = Instant::now() + Duration::from_millis(10);
    let future = Delay{when};
    // 类似JS 的Promise
    // await 时自身会调用 poll 来获取当前的状态
    // 如果 Ready 就切换到下一个状态机
    // Pending时调用返回，不切换到下一状态
    let out = future.await;
    assert_eq!(out, "done");
}

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        }else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

// 实际上面的Future 会被转义成下面模样
enum MainFuture {
    State0,
    State1(Delay),
    Terminated,
}

impl Future for MainFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        use MainFuture::*;
        #[allow(unused_doc_comments)]
        /**
        * 这种 Future 最容易理解，死循环一直轮询 poll()
        * 如果返回Ready就进入下个状态
        * 之后更优化的方式是将 poll 线程阻塞，然后由 walker 唤醒
        **/
        loop {
            match *self {
                State0 => {
                    let when = Instant::now() + Duration::from_millis(10);
                    let future = Delay{when};
                    *self = State1(future);
                }
                State1(ref mut my_future) => {
                    match Pin::new(my_future).poll(cx) {
                        Poll::Ready(out)=> {
                            assert_eq!(out, "done");
                            *self = Terminated;
                        }
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                    }
                }
                Terminated => {
                    panic!("future polled after completion")
                }
            }
        }
    }
}