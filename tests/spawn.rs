extern crate tokio_core;
extern crate env_logger;
extern crate futures;

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use futures::{Future, Poll};
use futures::future;
use futures::sync::oneshot;
use tokio_core::reactor::{Core, Timeout};

#[test]
fn simple() {
    drop(env_logger::init());
    let mut lp = Core::new().unwrap();

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();
    lp.handle().spawn(future::lazy(|| {
        tx1.send(1).unwrap();
        Ok(())
    }));
    lp.remote().spawn(|_| {
        future::lazy(|| {
            tx2.send(2).unwrap();
            Ok(())
        })
    });

    assert_eq!(lp.run(rx1.join(rx2)).unwrap(), (1, 2));
}

#[test]
fn simple_core_poll() {
    drop(env_logger::init());
    let mut lp = Core::new().unwrap();

    let (tx, rx) = mpsc::channel();
    let (tx1, tx2) = (tx.clone(), tx.clone());

    lp.turn(Some(Duration::new(0, 0)));
    lp.handle().spawn(future::lazy(move || {
        tx1.send(1).unwrap();
        Ok(())
    }));
    lp.turn(Some(Duration::new(0, 0)));
    lp.handle().spawn(future::lazy(move || {
        tx2.send(2).unwrap();
        Ok(())
    }));
    assert_eq!(rx.try_recv().unwrap(), 1);
    assert!(rx.try_recv().is_err());
    lp.turn(Some(Duration::new(0, 0)));
    assert_eq!(rx.try_recv().unwrap(), 2);
}

#[test]
fn spawn_in_poll() {
    drop(env_logger::init());
    let mut lp = Core::new().unwrap();

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();
    let remote = lp.remote();
    lp.handle().spawn(future::lazy(move || {
        tx1.send(1).unwrap();
        remote.spawn(|_| {
            future::lazy(|| {
                tx2.send(2).unwrap();
                Ok(())
            })
        });
        Ok(())
    }));

    assert_eq!(lp.run(rx1.join(rx2)).unwrap(), (1, 2));
}

#[test]
fn drop_timeout_in_spawn() {
    drop(env_logger::init());
    let mut lp = Core::new().unwrap();

    let (tx, rx) = oneshot::channel();
    let remote = lp.remote();
    thread::spawn(move || {
        remote.spawn(|handle| {
            drop(Timeout::new(Duration::new(1, 0), handle));
            tx.send(()).unwrap();
            Ok(())
        });
    });

    lp.run(rx).unwrap();
}

#[test]
fn spawn_in_drop() {
    drop(env_logger::init());
    let mut lp = Core::new().unwrap();

    let (tx, rx) = oneshot::channel();
    let remote = lp.remote();

    struct OnDrop<F: FnMut()>(F);

    impl<F: FnMut()> Drop for OnDrop<F> {
        fn drop(&mut self) {
            (self.0)();
        }
    }

    struct MyFuture {
        _data: Box<Any>,
    }

    impl Future for MyFuture {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Poll<(), ()> {
            Ok(().into())
        }
    }

    thread::spawn(move || {
        let mut tx = Some(tx);
        remote.spawn(|handle| {
            let handle = handle.clone();
            MyFuture {
                _data: Box::new(OnDrop(move || {
                    let mut tx = tx.take();
                    handle.spawn_fn(move || {
                        tx.take().unwrap().send(()).unwrap();
                        Ok(())
                    });
                })),

            }
        });
    });

    lp.run(rx).unwrap();
}

#[test]
fn run_background() {
    let mut core = Core::new().expect("Core::new");

    core.run_background(None);
    core.run_background(Some(Duration::from_secs(100000000)));

    let (tx, rx) = oneshot::channel();

    let executed = Arc::new(AtomicBool::new(false));
    let executed_copy = executed.clone();

    let future = rx
        .map(move |_| {
            executed_copy.store(true, Ordering::SeqCst)
        })
        .map_err(|_| unreachable!());

    core.handle().spawn(future);

    core.run_background(Some(Duration::from_millis(1)));

    thread::spawn(move || {
        tx.send(1).expect("send");
    });

    core.run_background(None);

    assert!(executed.load(Ordering::SeqCst));

    core.run_background(None);
}
