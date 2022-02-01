// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{
    active_open::ActiveOpenSocket, established::EstablishedSocket, isn_generator::IsnGenerator,
    passive_open::PassiveSocket,
};
use crate::{
    protocols::{
        arp::ArpPeer,
        ethernet2::{EtherType2, Ethernet2Header},
        ip,
        ip::EphemeralPorts,
        ipv4::{Ipv4Endpoint, Ipv4Header, Ipv4Protocol2},
        tcp::{
            operations::{AcceptFuture, ConnectFuture, ConnectFutureState, PopFuture, PushFuture},
            segment::{TcpHeader, TcpSegment},
        },
    },
    runtime::Runtime,
};
use futures::channel::mpsc;
use runtime::fail::Fail;
use runtime::queue::IoQueueDescriptor;
use runtime::RuntimeBuf;
use std::collections::HashMap;
use std::{
    cell::RefCell,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

#[cfg(feature = "profiler")]
use perftools::timer;

enum Socket {
    Inactive {
        local: Option<Ipv4Endpoint>,
    },
    Listening {
        local: Ipv4Endpoint,
    },
    Connecting {
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    },
    Established {
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    },
}

pub struct Inner<RT: Runtime> {
    isn_generator: IsnGenerator,

    ephemeral_ports: EphemeralPorts,

    // FD -> local port
    sockets: HashMap<IoQueueDescriptor, Socket>,

    passive: HashMap<Ipv4Endpoint, PassiveSocket<RT>>,
    connecting: HashMap<(Ipv4Endpoint, Ipv4Endpoint), ActiveOpenSocket<RT>>,
    established: HashMap<(Ipv4Endpoint, Ipv4Endpoint), EstablishedSocket<RT>>,

    rt: RT,
    arp: ArpPeer<RT>,

    dead_socket_tx: mpsc::UnboundedSender<IoQueueDescriptor>,
}

pub struct TcpPeer<RT: Runtime> {
    pub(super) inner: Rc<RefCell<Inner<RT>>>,
}

impl<RT: Runtime> TcpPeer<RT> {
    pub fn new(rt: RT, arp: ArpPeer<RT>) -> Self {
        let (tx, rx) = mpsc::unbounded();
        let inner = Rc::new(RefCell::new(Inner::new(rt.clone(), arp, tx, rx)));
        Self { inner }
    }

    /// Opens a TCP socket.
    pub fn do_socket(&self, fd: IoQueueDescriptor) {
        #[cfg(feature = "profiler")]
        timer!("tcp::socket");

        let mut inner = self.inner.borrow_mut();

        // Sanity check.
        assert_eq!(
            inner.sockets.contains_key(&fd),
            false,
            "file descriptor in use"
        );

        let socket = Socket::Inactive { local: None };
        inner.sockets.insert(fd, socket);
    }

    pub fn bind(&self, fd: IoQueueDescriptor, addr: Ipv4Endpoint) -> Result<(), Fail> {
        let mut inner = self.inner.borrow_mut();
        if addr.get_port() >= ip::Port::first_private_port() {
            return Err(Fail::Malformed {
                details: "Port number in private port range",
            });
        }
        match inner.sockets.get_mut(&fd) {
            Some(Socket::Inactive { ref mut local }) => {
                *local = Some(addr);
                Ok(())
            }
            _ => Err(Fail::Malformed {
                details: "Invalid file descriptor",
            }),
        }
    }

    pub fn receive(&self, ip_header: &Ipv4Header, buf: RT::Buf) -> Result<(), Fail> {
        self.inner.borrow_mut().receive(ip_header, buf)
    }

    pub fn listen(&self, fd: IoQueueDescriptor, backlog: usize) -> Result<(), Fail> {
        let mut inner = self.inner.borrow_mut();
        let local = match inner.sockets.get_mut(&fd) {
            Some(Socket::Inactive { local: Some(local) }) => *local,
            _ => {
                return Err(Fail::Malformed {
                    details: "Invalid file descriptor",
                })
            }
        };
        // TODO: Should this move to bind?
        if inner.passive.contains_key(&local) {
            return Err(Fail::ResourceBusy {
                details: "Port already in use",
            });
        }

        let socket = PassiveSocket::new(local, backlog, inner.rt.clone(), inner.arp.clone());
        assert!(inner.passive.insert(local, socket).is_none());
        inner.sockets.insert(fd, Socket::Listening { local });
        Ok(())
    }

    /// Accepts an incoming connection.
    pub fn do_accept(&self, fd: IoQueueDescriptor, newfd: IoQueueDescriptor) -> AcceptFuture<RT> {
        AcceptFuture {
            fd,
            newfd,
            inner: self.inner.clone(),
        }
    }

    /// Handles an incoming connection.
    pub fn poll_accept(
        &self,
        fd: IoQueueDescriptor,
        newfd: IoQueueDescriptor,
        ctx: &mut Context,
    ) -> Poll<Result<IoQueueDescriptor, Fail>> {
        let mut inner_ = self.inner.borrow_mut();
        let inner = &mut *inner_;

        let local = match inner.sockets.get(&fd) {
            Some(Socket::Listening { local }) => local,
            Some(..) => {
                return Poll::Ready(Err(Fail::Malformed {
                    details: "Socket not listening",
                }))
            }
            None => return Poll::Ready(Err(Fail::Malformed { details: "Bad FD" })),
        };
        let passive = inner
            .passive
            .get_mut(local)
            .expect("sockets/local inconsistency");
        let cb = match passive.poll_accept(ctx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Ok(e)) => e,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
        };
        let established = EstablishedSocket::new(cb, newfd, inner.dead_socket_tx.clone());
        let key = (established.cb.get_local(), established.cb.get_remote());

        let socket = Socket::Established {
            local: established.cb.get_local(),
            remote: established.cb.get_remote(),
        };
        assert!(inner.sockets.insert(newfd, socket).is_none());
        assert!(inner.established.insert(key, established).is_none());

        Poll::Ready(Ok(newfd))
    }

    pub fn connect(&self, fd: IoQueueDescriptor, remote: Ipv4Endpoint) -> ConnectFuture<RT> {
        let mut inner = self.inner.borrow_mut();

        let r = try {
            match inner.sockets.get_mut(&fd) {
                Some(Socket::Inactive { .. }) => (),
                _ => Err(Fail::Malformed {
                    details: "Invalid file descriptor",
                })?,
            }

            // TODO: We need to free these!
            let local_port = inner.ephemeral_ports.alloc()?;
            let local = Ipv4Endpoint::new(inner.rt.local_ipv4_addr(), local_port);

            let socket = Socket::Connecting { local, remote };
            inner.sockets.insert(fd, socket);

            let local_isn = inner.isn_generator.generate(&local, &remote);
            let key = (local, remote);
            let socket = ActiveOpenSocket::new(
                local_isn,
                local,
                remote,
                inner.rt.clone(),
                inner.arp.clone(),
            );
            assert!(inner.connecting.insert(key, socket).is_none());
            fd
        };
        let state = match r {
            Ok(..) => ConnectFutureState::InProgress,
            Err(e) => ConnectFutureState::Failed(e),
        };
        ConnectFuture {
            fd,
            state,
            inner: self.inner.clone(),
        }
    }

    pub fn poll_recv(
        &self,
        fd: IoQueueDescriptor,
        ctx: &mut Context,
    ) -> Poll<Result<RT::Buf, Fail>> {
        let inner = self.inner.borrow_mut();
        let key = match inner.sockets.get(&fd) {
            Some(Socket::Established { local, remote }) => (*local, *remote),
            Some(Socket::Connecting { .. }) => {
                return Poll::Ready(Err(Fail::Malformed {
                    details: "pool_recv(): socket connecting",
                }))
            }
            Some(Socket::Inactive { .. }) => {
                return Poll::Ready(Err(Fail::Malformed {
                    details: "pool_recv(): socket inactive",
                }))
            }
            Some(Socket::Listening { .. }) => {
                return Poll::Ready(Err(Fail::Malformed {
                    details: "pool_recv(): socket listening",
                }))
            }
            None => return Poll::Ready(Err(Fail::Malformed { details: "Bad FD" })),
        };
        match inner.established.get(&key) {
            Some(ref s) => s.poll_recv(ctx),
            None => Poll::Ready(Err(Fail::Malformed {
                details: "Socket not established",
            })),
        }
    }

    pub fn push(&self, fd: IoQueueDescriptor, buf: RT::Buf) -> PushFuture<RT> {
        let err = match self.send(fd, buf) {
            Ok(()) => None,
            Err(e) => Some(e),
        };
        PushFuture {
            fd,
            err,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn pop(&self, fd: IoQueueDescriptor) -> PopFuture<RT> {
        PopFuture {
            fd,
            inner: self.inner.clone(),
        }
    }

    fn send(&self, fd: IoQueueDescriptor, buf: RT::Buf) -> Result<(), Fail> {
        let inner = self.inner.borrow_mut();
        let key = match inner.sockets.get(&fd) {
            Some(Socket::Established { local, remote }) => (*local, *remote),
            Some(..) => {
                return Err(Fail::Malformed {
                    details: "Socket not established",
                })
            }
            None => return Err(Fail::Malformed { details: "Bad FD" }),
        };
        match inner.established.get(&key) {
            Some(ref s) => s.send(buf),
            None => Err(Fail::Malformed {
                details: "Socket not established",
            }),
        }
    }

    /// Closes a TCP socket.
    pub fn do_close(&self, fd: IoQueueDescriptor) -> Result<(), Fail> {
        let inner = self.inner.borrow_mut();

        match inner.sockets.get(&fd) {
            Some(Socket::Established { local, remote }) => {
                let key = (*local, *remote);
                match inner.established.get(&key) {
                    Some(ref s) => s.close()?,
                    None => {
                        return Err(Fail::Malformed {
                            details: "Socket not established",
                        })
                    }
                }
            }

            Some(..) => {
                return Err(Fail::Unsupported {
                    details: "implement close for listening sockets",
                })
            }
            None => return Err(Fail::Malformed { details: "Bad FD" }),
        }

        Ok(())
    }

    pub fn remote_mss(&self, fd: IoQueueDescriptor) -> Result<usize, Fail> {
        let inner = self.inner.borrow();
        let key = match inner.sockets.get(&fd) {
            Some(Socket::Established { local, remote }) => (*local, *remote),
            Some(..) => {
                return Err(Fail::Malformed {
                    details: "Socket not established",
                })
            }
            None => return Err(Fail::Malformed { details: "Bad FD" }),
        };
        match inner.established.get(&key) {
            Some(ref s) => Ok(s.remote_mss()),
            None => Err(Fail::Malformed {
                details: "Socket not established",
            }),
        }
    }

    pub fn current_rto(&self, fd: IoQueueDescriptor) -> Result<Duration, Fail> {
        let inner = self.inner.borrow();
        let key = match inner.sockets.get(&fd) {
            Some(Socket::Established { local, remote }) => (*local, *remote),
            Some(..) => {
                return Err(Fail::Malformed {
                    details: "Socket not established",
                })
            }
            None => return Err(Fail::Malformed { details: "Bad FD" }),
        };
        match inner.established.get(&key) {
            Some(ref s) => Ok(s.current_rto()),
            None => Err(Fail::Malformed {
                details: "Socket not established",
            }),
        }
    }

    pub fn endpoints(&self, fd: IoQueueDescriptor) -> Result<(Ipv4Endpoint, Ipv4Endpoint), Fail> {
        let inner = self.inner.borrow();
        let key = match inner.sockets.get(&fd) {
            Some(Socket::Established { local, remote }) => (*local, *remote),
            Some(..) => {
                return Err(Fail::Malformed {
                    details: "Socket not established",
                })
            }
            None => return Err(Fail::Malformed { details: "Bad FD" }),
        };
        match inner.established.get(&key) {
            Some(ref s) => Ok(s.endpoints()),
            None => Err(Fail::Malformed {
                details: "Socket not established",
            }),
        }
    }
}

impl<RT: Runtime> Inner<RT> {
    fn new(
        rt: RT,
        arp: ArpPeer<RT>,
        dead_socket_tx: mpsc::UnboundedSender<IoQueueDescriptor>,
        _dead_socket_rx: mpsc::UnboundedReceiver<IoQueueDescriptor>,
    ) -> Self {
        Self {
            isn_generator: IsnGenerator::new(rt.rng_gen()),
            ephemeral_ports: EphemeralPorts::new(&rt),
            sockets: HashMap::new(),
            passive: HashMap::new(),
            connecting: HashMap::new(),
            established: HashMap::new(),
            rt,
            arp,
            dead_socket_tx,
        }
    }

    fn receive(&mut self, ip_hdr: &Ipv4Header, buf: RT::Buf) -> Result<(), Fail> {
        let tcp_options = self.rt.tcp_options();
        let (tcp_hdr, data) = TcpHeader::parse(ip_hdr, buf, tcp_options.rx_checksum_offload())?;
        debug!("TCP received {:?}", tcp_hdr);
        let local = Ipv4Endpoint::new(ip_hdr.dst_addr(), tcp_hdr.dst_port);
        let remote = Ipv4Endpoint::new(ip_hdr.src_addr(), tcp_hdr.src_port);

        if remote.get_address().is_broadcast()
            || remote.get_address().is_multicast()
            || remote.get_address().is_unspecified()
        {
            return Err(Fail::Malformed {
                details: "Invalid address type",
            });
        }
        let key = (local, remote);

        if let Some(s) = self.established.get(&key) {
            debug!("Routing to established connection: {:?}", key);
            s.receive(&tcp_hdr, data);
            return Ok(());
        }
        if let Some(s) = self.connecting.get_mut(&key) {
            debug!("Routing to connecting connection: {:?}", key);
            s.receive(&tcp_hdr);
            return Ok(());
        }
        let (local, _) = key;
        if let Some(s) = self.passive.get_mut(&local) {
            debug!("Routing to passive connection: {:?}", local);
            return s.receive(ip_hdr, &tcp_hdr);
        }

        // The packet isn't for an open port; send a RST segment.
        debug!("Sending RST for {:?}, {:?}", local, remote);
        self.send_rst(&local, &remote)?;
        Ok(())
    }

    fn send_rst(&mut self, local: &Ipv4Endpoint, remote: &Ipv4Endpoint) -> Result<(), Fail> {
        // TODO: Make this work pending on ARP resolution if needed.
        let remote_link_addr =
            self.arp
                .try_query(remote.get_address())
                .ok_or(Fail::ResourceNotFound {
                    details: "RST destination not in ARP cache",
                })?;

        let mut tcp_hdr = TcpHeader::new(local.get_port(), remote.get_port());
        tcp_hdr.rst = true;

        let segment = TcpSegment {
            ethernet2_hdr: Ethernet2Header::new(
                remote_link_addr,
                self.rt.local_link_addr(),
                EtherType2::Ipv4,
            ),
            ipv4_hdr: Ipv4Header::new(
                local.get_address(),
                remote.get_address(),
                Ipv4Protocol2::Tcp,
            ),
            tcp_hdr,
            data: RT::Buf::empty(),
            tx_checksum_offload: self.rt.tcp_options().tx_checksum_offload(),
        };
        self.rt.transmit(segment);

        Ok(())
    }

    pub(super) fn poll_connect_finished(
        &mut self,
        fd: IoQueueDescriptor,
        context: &mut Context,
    ) -> Poll<Result<(), Fail>> {
        let key = match self.sockets.get(&fd) {
            Some(Socket::Connecting { local, remote }) => (*local, *remote),
            Some(..) => {
                return Poll::Ready(Err(Fail::Malformed {
                    details: "Socket not connecting",
                }))
            }
            None => return Poll::Ready(Err(Fail::Malformed { details: "Bad FD" })),
        };

        let result = {
            let socket = match self.connecting.get_mut(&key) {
                Some(s) => s,
                None => {
                    return Poll::Ready(Err(Fail::Malformed {
                        details: "Socket not connecting",
                    }))
                }
            };
            match socket.poll_result(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(r) => r,
            }
        };
        self.connecting.remove(&key);

        let cb = result?;
        let socket = EstablishedSocket::new(cb, fd, self.dead_socket_tx.clone());
        assert!(self.established.insert(key, socket).is_none());
        let (local, remote) = key;
        self.sockets
            .insert(fd, Socket::Established { local, remote });

        Poll::Ready(Ok(()))
    }
}
