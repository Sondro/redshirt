// Copyright (C) 2019  Pierre Krieger
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use futures::prelude::*;
use hashbrown::{hash_map::Entry, HashMap};
use std::{fmt, hash::Hash, iter, net::SocketAddr, pin::Pin, sync::MutexGuard};

mod interface;

/// State machine managing all the network interfaces and sockets.
///
/// The `TIfId` generic parameter is an identifier for network interfaces.
pub struct NetworkManager<TIfId, TIfUser> {
    devices: HashMap<TIfId, Device<TIfUser>>,
}

/// State of a device.
struct Device<TIfUser> {
    /// Inner state.
    inner: interface::NetInterfaceState,
    /// Additional user data.
    user_data: TIfUser,
}

/// Event generated by the [`NetworkManagerEvent::next_event`] function.
#[derive(Debug)]
pub enum NetworkManagerEvent<'a, TIfId, TIfUser> {
    /// Data to be sent out by the Ethernet cable is available.
    ///
    /// Contains a mutable reference of the data buffer. Data can be left in the buffer if
    /// desired.
    EthernetCableOut(TIfId, &'a mut TIfUser, MutexGuard<'a, Vec<u8>>),
    /// A TCP/IP socket has connected to its target.
    TcpConnected(TcpSocket<'a, TIfId>),
    /// A TCP/IP socket has been closed by the remote.
    TcpClosed(TcpSocket<'a, TIfId>),
    /// A TCP/IP socket has data ready to be read.
    TcpReadReady(TcpSocket<'a, TIfId>),
    /// A TCP/IP socket has finished writing the data that we passed to it, and is now ready to
    /// accept more.
    TcpWriteFinished(TcpSocket<'a, TIfId>),
}

pub struct TcpSocket<'a, TIfId> {
    inner: interface::TcpSocket<'a>,
    device_id: TIfId,
}

/// Identifier of a socket within the [`NetworkManager`]. Common between all types of sockets.
#[derive(Debug, Copy, Clone, PartialEq, Eq)] // TODO: Hash
pub struct SocketId<TIfId> {
    interface: TIfId,
    socket: interface::SocketId,
}

impl<TIfId, TIfUser> NetworkManager<TIfId, TIfUser>
where
    TIfId: Clone + Hash + PartialEq + Eq,
{
    pub fn new() -> Self {
        NetworkManager {
            devices: HashMap::new(),
        }
    }

    pub fn build_tcp_socket(&mut self, listen: bool, addr: &SocketAddr) -> TcpSocket<TIfId> {
        for (device_id, device) in self.devices.iter_mut() {
            if let Ok(socket) = device.inner.build_tcp_socket(listen, addr) {
                return TcpSocket {
                    inner: socket,
                    device_id: device_id.clone(),
                };
            }
        }

        panic!() // TODO:
    }

    pub fn tcp_socket_by_id(&mut self, id: &SocketId<TIfId>) -> Option<TcpSocket<TIfId>> {
        let interface = &mut self.devices.get_mut(&id.interface)?.inner;
        let inner = interface.tcp_socket_by_id(id.socket)?;
        Some(TcpSocket {
            inner,
            device_id: id.interface.clone(),
        })
    }

    /// Registers an interface with the given ID. Returns an error if an interface with that ID
    /// already exists.
    pub fn register_interface(&mut self, id: TIfId, mac_address: [u8; 6], user_data: TIfUser) -> Result<(), ()> {
        let entry = match self.devices.entry(id) {
            Entry::Occupied(_) => return Err(()),
            Entry::Vacant(e) => e,
        };

        let interface = interface::NetInterfaceStateBuilder::default()
            .with_ip_addr("192.168.1.20".parse().unwrap(), 24)  // TODO: hack
            .with_ip_addr("fe80::9d39:1765:52bd:8383".parse().unwrap(), 64)  // TODO: hack
            .with_mac_address(mac_address)
            .build();
        entry.insert(Device {
            inner: interface,
            user_data,
        });
        Ok(())
    }

    // TODO: better API?
    pub fn unregister_interface(&mut self, id: &TIfId) {
        let device = self.devices.remove(id);
        // TODO:
    }

    /// Extract the data to transmit out of the Ethernet cable.
    ///
    /// Returns an empty buffer if nothing is ready.
    // TODO: better API?
    pub fn interface_user_data(&mut self, id: &TIfId) -> &mut TIfUser {
        &mut self.devices
            .get_mut(id)
            .unwrap() // TODO: don't unwrap
            .user_data
    }

    /// Extract the data to transmit out of the Ethernet cable.
    ///
    /// Returns an empty buffer if nothing is ready.
    // TODO: better API?
    pub fn read_ethernet_cable_out(&mut self, id: &TIfId) -> Vec<u8> {
        self.devices
            .get_mut(id)
            .unwrap() // TODO: don't unwrap
            .inner
            .read_ethernet_cable_out()
    }

    /// Injects some data coming from the Ethernet cable.
    // TODO: better API?
    pub fn inject_interface_data(&mut self, id: &TIfId, data: impl AsRef<[u8]>) {
        self.devices
            .get_mut(id)
            .unwrap() // TODO: don't unwrap
            .inner
            .inject_interface_data(data)
    }

    /// Returns the next event generated by the [`NetworkManager`].
    pub async fn next_event<'a>(&'a mut self) -> NetworkManagerEvent<'a, TIfId, TIfUser> {
        // TODO: optimize?
        let next_event = future::select_all(
            self.devices
                .iter_mut()
                .map(move |(n, d)| {
                    let user_data = &mut d.user_data;
                    Box::pin(d.inner.next_event().map(move |ev| (n.clone(), user_data, ev))) as Pin<Box<dyn Future<Output = _>>>
                })
                .chain(iter::once(Box::pin(future::pending()) as Pin<Box<_>>)),
        );
        match next_event.await.0 {
            (device_id, user_data, interface::NetInterfaceEvent::EthernetCableOut(buffer)) => {
                NetworkManagerEvent::EthernetCableOut(device_id, user_data, buffer)
            }
            (device_id, _, interface::NetInterfaceEvent::TcpConnected(inner)) => {
                NetworkManagerEvent::TcpConnected(TcpSocket { inner, device_id })
            }
            (device_id, _, interface::NetInterfaceEvent::TcpClosed(inner)) => {
                NetworkManagerEvent::TcpClosed(TcpSocket { inner, device_id })
            }
            (device_id, _, interface::NetInterfaceEvent::TcpReadReady(inner)) => {
                NetworkManagerEvent::TcpReadReady(TcpSocket { inner, device_id })
            }
            (device_id, _, interface::NetInterfaceEvent::TcpWriteFinished(inner)) => {
                NetworkManagerEvent::TcpWriteFinished(TcpSocket { inner, device_id })
            }
        }
    }
}

impl<'a, TIfId> fmt::Debug for TcpSocket<'a, TIfId>
where
    TIfId: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("TcpSocket").field(&self.device_id).finish()
    }
}

impl<'a, TIfId: Clone> TcpSocket<'a, TIfId> {
    /// Returns the identifier of the socket, for later retrieval.
    pub fn id(&self) -> SocketId<TIfId> {
        SocketId {
            interface: self.device_id.clone(),
            socket: self.inner.id(),
        }
    }

    /// Closes the socket.
    pub fn close(self) {
        //self.device.
    }
}
