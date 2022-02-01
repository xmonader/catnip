// // Copyright (c) Microsoft Corporation.
// // Licensed under the MIT license.

use crate::{
    protocols::{ip, ipv4::Ipv4Endpoint},
    test_helpers,
};
use futures::task::{noop_waker_ref, Context};
use runtime::fail::Fail;
use runtime::memory::BytesMut;
use runtime::queue::IoQueueDescriptor;
use std::{
    convert::TryFrom,
    future::Future,
    pin::Pin,
    task::Poll,
    time::{Duration, Instant},
};

//==============================================================================
// Bind & Close
//==============================================================================

#[test]
fn udp_bind_udp_close() {
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    now += Duration::from_micros(1);

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    bob.udp_close(bob_fd).unwrap();
}

//==============================================================================
// Push & Pop
//==============================================================================

#[test]
fn udp_push_pop() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    // Send data to Bob.
    let buf = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    alice.udp_pushto(alice_fd, buf.clone(), bob_addr).unwrap();
    alice.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Alice.
    bob.receive(alice.rt().pop_frame()).unwrap();
    let mut pop_future = bob.udp_pop(bob_fd);
    let (remote_addr, received_buf) = match Future::poll(Pin::new(&mut pop_future), &mut ctx) {
        Poll::Ready(Ok((Some(remote_addr), received_buf))) => Ok((remote_addr, received_buf)),
        _ => Err(()),
    }
    .unwrap();
    assert_eq!(remote_addr, alice_addr);
    assert_eq!(received_buf, buf);

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    bob.udp_close(bob_fd).unwrap();
}

//==============================================================================
// Ping Pong
//==============================================================================

#[test]
fn udp_ping_pong() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    // Send data to Bob.
    let buf_a = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    alice.udp_pushto(alice_fd, buf_a.clone(), bob_addr).unwrap();
    alice.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Alice.
    bob.receive(alice.rt().pop_frame()).unwrap();
    let mut pop_future = bob.udp_pop(bob_fd);
    let (remote_addr, received_buf_a) = match Future::poll(Pin::new(&mut pop_future), &mut ctx) {
        Poll::Ready(Ok((Some(remote_addr), received_buf_a))) => Ok((remote_addr, received_buf_a)),
        _ => Err(()),
    }
    .unwrap();
    assert_eq!(remote_addr, alice_addr);
    assert_eq!(received_buf_a, buf_a);

    now += Duration::from_micros(1);

    // Send data to Alice.
    let buf_b = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    bob.udp_pushto(bob_fd, buf_b.clone(), alice_addr).unwrap();
    bob.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Bob.
    alice.receive(bob.rt().pop_frame()).unwrap();
    let mut pop_future = alice.udp_pop(alice_fd);
    let (remote_addr, received_buf_b) = match Future::poll(Pin::new(&mut pop_future), &mut ctx) {
        Poll::Ready(Ok((Some(remote_addr), received_buf_b))) => Ok((remote_addr, received_buf_b)),
        _ => Err(()),
    }
    .unwrap();
    assert_eq!(remote_addr, bob_addr);
    assert_eq!(received_buf_b, buf_b);

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    bob.udp_close(bob_fd).unwrap();
}

//==============================================================================
// Loop Bind & Close
//==============================================================================

#[test]
fn udp_loop1_bind_udp_close() {
    // Loop.
    for _ in 0..1000 {
        udp_bind_udp_close();
    }
}

#[test]
fn udp_loop2_bind_udp_close() {
    let mut now = Instant::now();

    // Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);

    // Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);

    // Loop.
    for _ in 0..1000 {
        // Bind Alice.
        let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
        alice.udp_bind(alice_fd, alice_addr).unwrap();

        // Bind bob.
        let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
        bob.udp_bind(bob_fd, bob_addr).unwrap();

        now += Duration::from_micros(1);

        // Close peers.
        alice.udp_close(alice_fd).unwrap();
        bob.udp_close(bob_fd).unwrap();
    }
}

//==============================================================================
// Loop Push & Pop
//==============================================================================

#[test]
fn udp_loop1_push_pop() {
    // Loop.
    for _ in 0..1000 {
        udp_push_pop();
    }
}

#[test]
fn udp_loop2_push_pop() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();
    // Loop.
    for b in 0..1000 {
        // Send data to Bob.
        let buf = BytesMut::from(&vec![(b % 256) as u8; 32][..]).freeze();
        alice.udp_pushto(alice_fd, buf.clone(), bob_addr).unwrap();
        alice.rt().poll_scheduler();

        now += Duration::from_micros(1);

        // Receive data from Alice.
        bob.receive(alice.rt().pop_frame()).unwrap();
        let mut pop_future = bob.udp_pop(bob_fd);
        let (remote_addr, received_buf) = match Future::poll(Pin::new(&mut pop_future), &mut ctx) {
            Poll::Ready(Ok((Some(remote_addr), received_buf))) => Ok((remote_addr, received_buf)),
            _ => Err(()),
        }
        .unwrap();
        assert_eq!(remote_addr, alice_addr);
        assert_eq!(received_buf, buf);
    }

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    bob.udp_close(bob_fd).unwrap();
}

//==============================================================================
// Loop Ping Pong
//==============================================================================

#[test]
fn udp_loop1_ping_pong() {
    // Loop.
    for _ in 0..1000 {
        udp_ping_pong();
    }
}

#[test]
fn udp_loop2_ping_pong() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();
    //
    // Loop.
    for _ in 0..1000 {
        // Send data to Bob.
        let buf_a = BytesMut::from(&vec![0x5a; 32][..]).freeze();
        alice.udp_pushto(alice_fd, buf_a.clone(), bob_addr).unwrap();
        alice.rt().poll_scheduler();

        now += Duration::from_micros(1);

        // Receive data from Alice.
        bob.receive(alice.rt().pop_frame()).unwrap();
        let mut pop_future = bob.udp_pop(bob_fd);
        let (remote_addr, received_buf_a) = match Future::poll(Pin::new(&mut pop_future), &mut ctx)
        {
            Poll::Ready(Ok((Some(remote_addr), received_buf_a))) => {
                Ok((remote_addr, received_buf_a))
            }
            _ => Err(()),
        }
        .unwrap();
        assert_eq!(remote_addr, alice_addr);
        assert_eq!(received_buf_a, buf_a);

        now += Duration::from_micros(1);

        // Send data to Alice.
        let buf_b = BytesMut::from(&vec![0x5a; 32][..]).freeze();
        bob.udp_pushto(bob_fd, buf_b.clone(), alice_addr).unwrap();
        bob.rt().poll_scheduler();

        now += Duration::from_micros(1);

        // Receive data from Bob.
        alice.receive(bob.rt().pop_frame()).unwrap();
        let mut pop_future = alice.udp_pop(alice_fd);
        let (remote_addr, received_buf_b) = match Future::poll(Pin::new(&mut pop_future), &mut ctx)
        {
            Poll::Ready(Ok((Some(remote_addr), received_buf_b))) => {
                Ok((remote_addr, received_buf_b))
            }
            _ => Err(()),
        }
        .unwrap();
        assert_eq!(remote_addr, bob_addr);
        assert_eq!(received_buf_b, buf_b);
    }

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    bob.udp_close(bob_fd).unwrap();
}

//==============================================================================
// Bad Bind
//==============================================================================

#[test]
fn udp_bind_address_in_use() {
    let now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Try to bind Alice again.
    match alice.udp_bind(alice_fd, alice_addr) {
        Err(Fail::Malformed {
            details: "Port already listening",
        }) => Ok(()),
        _ => Err(()),
    }
    .unwrap();

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
}

#[test]
fn udp_bind_bad_file_descriptor() {
    let now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = IoQueueDescriptor::try_from(usize::MAX).unwrap();

    // Try to bind Alice.
    match alice.udp_bind(alice_fd, alice_addr) {
        Err(Fail::BadFileDescriptor {}) => Ok(()),
        _ => Err(()),
    }
    .unwrap();
}

//==============================================================================
// Bad Close
//==============================================================================

#[test]
fn udp_udp_close_bad_file_descriptor() {
    let now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Try to udp_close bad file descriptor.
    match alice.udp_close(IoQueueDescriptor::try_from(usize::MAX).unwrap()) {
        Err(Fail::BadFileDescriptor {}) => Ok(()),
        _ => Err(()),
    }
    .unwrap();

    // Try to udp_close Alice two times.
    alice.udp_close(alice_fd).unwrap();
    match alice.udp_close(alice_fd) {
        Err(Fail::BadFileDescriptor {}) => Ok(()),
        _ => Err(()),
    }
    .unwrap();
}

//==============================================================================
// Bad Pop
//==============================================================================

#[test]
fn udp_pop_not_bound() {
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    // Bob does not create a socket.

    // Send data to Bob.
    let buf = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    alice.udp_pushto(alice_fd, buf.clone(), bob_addr).unwrap();
    alice.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Alice.
    match bob.receive(alice.rt().pop_frame()) {
        Err(Fail::Malformed {
            details: "Port not bound",
        }) => Ok(()),
        _ => Err(()),
    }
    .unwrap();

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    // Bob does not have a socket.
}

//==============================================================================
// Bad Push
//==============================================================================

#[test]
fn udp_push_bad_file_descriptor() {
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = Ipv4Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: IoQueueDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = Ipv4Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: IoQueueDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    // Send data to Bob.
    let buf = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    match alice.udp_pushto(
        IoQueueDescriptor::try_from(usize::MAX).unwrap(),
        buf.clone(),
        bob_addr,
    ) {
        Err(Fail::BadFileDescriptor {}) => Ok(()),
        _ => Err(()),
    }
    .unwrap();

    alice.rt().poll_scheduler();
    now += Duration::from_micros(1);

    // Close peers.
    alice.udp_close(alice_fd).unwrap();
    bob.udp_close(bob_fd).unwrap();
}
