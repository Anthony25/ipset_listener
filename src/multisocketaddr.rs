
use std;
use std::io;
use std::net::{self, ToSocketAddrs};

/// MultiSocketAddr is here to store several SocketAddr and implement
/// to_socket_addrs in order to iterate over them.
pub struct MultiSocketAddr {
    /// Vector storing our list of SocketAddr
    addrs: Vec<net::SocketAddr>
}

impl ToSocketAddrs for MultiSocketAddr {
    type Iter = std::vec::IntoIter<net::SocketAddr>;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
        Ok(self.addrs.clone().into_iter())
    }
}

impl MultiSocketAddr {
    pub fn new() -> MultiSocketAddr {
        MultiSocketAddr {
            addrs: Vec::new(),
        }
    }
    /// Push a SocketAddr into self.addrs
    pub fn add<A: ToSocketAddrs>(&mut self, a: A) -> io::Result<()> {
        let addrs = try!(a.to_socket_addrs());
        self.addrs.extend(addrs);
        Ok(())
    }
}
