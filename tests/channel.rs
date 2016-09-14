extern crate tokio_core;
extern crate futures;

use tokio_core::reactor::Core;
use tokio_core::channel::channel;

use futures::stream::Stream;


#[test]
fn recv_after_core_drop() {
    let core: Core = Core::new().unwrap();

    let (_sender, receiver) = channel::<u32>(&core.handle()).unwrap();

    drop(core);

    assert!(receiver.wait().next().unwrap().is_err());
}

#[test]
fn recv_after_core_and_sender_drop() {
    let core: Core = Core::new().unwrap();

    let (sender, receiver) = channel::<u32>(&core.handle()).unwrap();

    drop(core);
    drop(sender);

    // although core is dropped, receiver still properly returns EOF
    assert!(receiver.wait().next().is_none());
}
