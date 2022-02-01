// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

pub mod engine;
pub mod runtime;

pub use self::runtime::TestRuntime;
pub use engine::Engine;

use crate::protocols::{arp::ArpConfig, ethernet2::MacAddress, tcp, udp};
use std::{
    collections::HashMap,
    net::Ipv4Addr,
    time::{Duration, Instant},
};

//==============================================================================
// Constants
//==============================================================================

pub const RECEIVE_WINDOW_SIZE: usize = 1024;
pub const ALICE_MAC: MacAddress = MacAddress::new([0x12, 0x23, 0x45, 0x67, 0x89, 0xab]);
pub const ALICE_IPV4: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 1);
pub const BOB_MAC: MacAddress = MacAddress::new([0xab, 0x89, 0x67, 0x45, 0x23, 0x12]);
pub const BOB_IPV4: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 2);
pub const CARRIE_MAC: MacAddress = MacAddress::new([0xef, 0xcd, 0xab, 0x89, 0x67, 0x45]);
pub const CARRIE_IPV4: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 3);

//==============================================================================
// Types
//==============================================================================

pub type TestEngine = Engine<TestRuntime>;

//==============================================================================
// Standalone Functions
//==============================================================================

pub fn new_alice(now: Instant) -> Engine<TestRuntime> {
    let arp_options = ArpConfig::new(
        Duration::from_secs(600),
        Duration::from_secs(1),
        2,
        HashMap::new(),
        false,
    );
    let udp_options = udp::UdpConfig::default();
    let tcp_options = tcp::Options::<TestRuntime>::default();
    let rt = TestRuntime::new(
        "alice",
        now,
        arp_options,
        udp_options,
        tcp_options,
        ALICE_MAC,
        ALICE_IPV4,
    );
    Engine::new(rt).unwrap()
}

pub fn new_bob(now: Instant) -> Engine<TestRuntime> {
    let arp_options = ArpConfig::new(
        Duration::from_secs(600),
        Duration::from_secs(1),
        2,
        HashMap::new(),
        false,
    );
    let udp_options = udp::UdpConfig::default();
    let tcp_options = tcp::Options::<TestRuntime>::default();
    let rt = TestRuntime::new(
        "bob",
        now,
        arp_options,
        udp_options,
        tcp_options,
        BOB_MAC,
        BOB_IPV4,
    );
    Engine::new(rt).unwrap()
}

pub fn new_alice2(now: Instant) -> Engine<TestRuntime> {
    let mut arp: HashMap<Ipv4Addr, MacAddress> = HashMap::<Ipv4Addr, MacAddress>::new();
    arp.insert(ALICE_IPV4, ALICE_MAC);
    arp.insert(BOB_IPV4, BOB_MAC);
    let arp_options = ArpConfig::new(
        Duration::from_secs(600),
        Duration::from_secs(1),
        2,
        arp,
        false,
    );
    let udp_options = udp::UdpConfig::default();
    let tcp_options = tcp::Options::<TestRuntime>::default();
    let rt = TestRuntime::new(
        "alice",
        now,
        arp_options,
        udp_options,
        tcp_options,
        ALICE_MAC,
        ALICE_IPV4,
    );
    Engine::new(rt).unwrap()
}

pub fn new_bob2(now: Instant) -> Engine<TestRuntime> {
    let mut arp: HashMap<Ipv4Addr, MacAddress> = HashMap::<Ipv4Addr, MacAddress>::new();
    arp.insert(BOB_IPV4, BOB_MAC);
    arp.insert(ALICE_IPV4, ALICE_MAC);
    let arp_options = ArpConfig::new(
        Duration::from_secs(600),
        Duration::from_secs(1),
        2,
        arp,
        false,
    );
    let udp_options = udp::UdpConfig::default();
    let tcp_options = tcp::Options::<TestRuntime>::default();
    let rt = TestRuntime::new(
        "bob",
        now,
        arp_options,
        udp_options,
        tcp_options,
        BOB_MAC,
        BOB_IPV4,
    );
    Engine::new(rt).unwrap()
}

pub fn new_carrie(now: Instant) -> Engine<TestRuntime> {
    let arp_options = ArpConfig::new(
        Duration::from_secs(600),
        Duration::from_secs(1),
        2,
        HashMap::new(),
        false,
    );
    let udp_options = udp::UdpConfig::default();
    let tcp_options = tcp::Options::<TestRuntime>::default();

    let rt = TestRuntime::new(
        "carrie",
        now,
        arp_options,
        udp_options,
        tcp_options,
        CARRIE_MAC,
        CARRIE_IPV4,
    );
    Engine::new(rt).unwrap()
}
