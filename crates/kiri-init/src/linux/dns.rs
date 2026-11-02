//! The embedded DNS server for user-defined networks, as Docker has: at
//! 127.0.0.11:53 inside each container's network namespace (what its
//! resolv.conf names, as with Docker), and on each bridge's gateway address,
//! port 53. Container names and aliases on the container's networks resolve
//! to their addresses (A, and AAAA on an IPv6 network); everything else is
//! forwarded to the VM's upstream resolver (or the container's own `--dns`
//! servers) and the answer relayed back. The 127.0.0.11 socket is opened in
//! the container's namespace by this agent, so no process runs in there.

use std::collections::HashMap;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::time::{Duration, Instant};

use super::sys;

const TTL: u32 = 600;
/// Names every container's /etc/hosts has for the host. They're answered here
/// too, as Docker Desktop's DNS does, for what reads DNS rather than
/// /etc/hosts: a kind or k3s node's CoreDNS forwards its pods' lookups here.
const HOST_NAMES: &[&str] = &["host.docker.internal", "host.kiri.internal", "gateway.docker.internal"];
/// Forwarded queries not answered within this are dropped.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(5);
/// At most this many forwarded queries in flight (each has its own socket);
/// more are dropped, and the client asks again.
const MAX_PENDING: usize = 256;

/// Docker's embedded DNS address, inside each container.
pub const EMBEDDED: [u8; 4] = [127, 0, 0, 11];

struct Server {
    /// The networks whose names it answers: a gateway's one, or every
    /// network of the container it is in.
    bridges: Vec<String>,
    sock: OwnedFd,
    /// In a container's namespace: (its PID, the address its `--dns`
    /// servers are kept under). Every query there comes from 127.0.0.1.
    container: Option<(i32, [u8; 4])>,
}

struct Pending {
    /// Its own socket, connected to the resolver: a fresh source port for
    /// every query, as Docker's embedded DNS does, so one lost or blocked
    /// flow doesn't take the resolver away from every later query, and an
    /// answer has to guess the port as well as the ID.
    sock: OwnedFd,
    /// The ID it went out with.
    our: u16,
    /// The server's socket the query came in on.
    server: RawFd,
    client: libc::sockaddr_in,
    id: u16,
    question: Vec<u8>,
    sent: Instant,
}

/// One name's IPv6 addresses, each with its container's IPv4 address.
type Owned6 = Vec<([u8; 4], [u8; 16])>;

#[derive(Default)]
pub struct Dns {
    servers: Vec<Server>,
    /// bridge → name (lowercase) → addresses.
    names: HashMap<String, HashMap<String, Vec<[u8; 4]>>>,
    /// bridge → name → (the container's IPv4 address there, its IPv6 one):
    /// the IPv4 address is what `unregister` names the container by.
    names6: HashMap<String, HashMap<String, Owned6>>,
    /// Container address → its `--dns` servers, used instead of the VM's.
    clients: HashMap<[u8; 4], Vec<[u8; 4]>>,
    pending: Vec<Pending>,
}

fn udp_socket() -> io::Result<OwnedFd> {
    // SAFETY: plain socket creation; owned immediately.
    let fd = sys::check(
        unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0) } as _,
        "dns socket",
    )?;
    // SAFETY: `fd` is fresh and ours.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
}

fn sockaddr(ip: [u8; 4], port: u16) -> libc::sockaddr_in {
    // SAFETY: sockaddr_in is plain data; all-zero is valid.
    let mut a: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    a.sin_family = libc::AF_INET as libc::sa_family_t;
    a.sin_port = port.to_be();
    a.sin_addr.s_addr = u32::from_ne_bytes(ip);
    a
}

fn send_to(fd: RawFd, data: &[u8], to: &libc::sockaddr_in) {
    // SAFETY: sending a buffer we own to a valid sockaddr_in.
    unsafe {
        libc::sendto(
            fd,
            data.as_ptr().cast(),
            data.len(),
            0,
            (to as *const libc::sockaddr_in).cast(),
            std::mem::size_of::<libc::sockaddr_in>() as u32,
        )
    };
}

impl Dns {
    /// Starts answering on a bridge's gateway address (once per bridge).
    pub fn serve(&mut self, bridge: &str, gateway: [u8; 4]) -> io::Result<()> {
        if self.servers.iter().any(|s| s.container.is_none() && s.bridges.iter().any(|b| b == bridge)) {
            return Ok(());
        }
        let sock = bound(gateway)?;
        self.servers.push(Server { bridges: vec![bridge.to_owned()], sock, container: None });
        Ok(())
    }

    /// Starts answering at 127.0.0.11:53 in container `pid`'s network
    /// namespace `netns` (lo up) for the names on `bridge`, or adds `bridge`
    /// to the networks its server answers for. `owner` is the address its
    /// `--dns` servers are kept under (`set_upstream`).
    pub fn serve_in(&mut self, netns: &std::fs::File, pid: i32, owner: [u8; 4], bridge: &str) -> io::Result<()> {
        if let Some(s) = self.servers.iter_mut().find(|s| s.container.is_some_and(|(p, _)| p == pid)) {
            if !s.bridges.iter().any(|b| b == bridge) {
                s.bridges.push(bridge.to_owned());
            }
            return Ok(());
        }
        let sock = super::netlink::in_netns(netns, || bound(EMBEDDED))?;
        self.servers.push(Server { bridges: vec![bridge.to_owned()], sock, container: Some((pid, owner)) });
        Ok(())
    }

    /// Container `pid` left `bridge` (`network disconnect`).
    pub fn leave(&mut self, pid: i32, bridge: &str) {
        for s in self.servers.iter_mut().filter(|s| s.container.is_some_and(|(p, _)| p == pid)) {
            s.bridges.retain(|b| b != bridge);
        }
    }

    /// Container `pid` is gone: its 127.0.0.11 server goes.
    pub fn drop_container(&mut self, pid: i32) {
        self.servers.retain(|s| !s.container.is_some_and(|(p, _)| p == pid));
        self.forget_pending();
    }

    /// Forwarded queries whose server is gone go too, so a reused
    /// descriptor never gets another server's answers.
    fn forget_pending(&mut self) {
        let servers = &self.servers;
        self.pending.retain(|p| servers.iter().any(|s| s.sock.as_raw_fd() == p.server));
    }

    pub fn register(&mut self, bridge: &str, names: impl Iterator<Item = impl AsRef<str>>, ip: [u8; 4]) {
        let table = self.names.entry(bridge.to_owned()).or_default();
        for n in names {
            let list = table.entry(n.as_ref().to_ascii_lowercase()).or_default();
            if !list.contains(&ip) {
                list.push(ip);
            }
        }
    }

    /// The IPv6 address of the container that has `owner` on this network.
    pub fn register6(&mut self, bridge: &str, names: impl Iterator<Item = impl AsRef<str>>, owner: [u8; 4], ip: [u8; 16]) {
        let table = self.names6.entry(bridge.to_owned()).or_default();
        for n in names {
            let list = table.entry(n.as_ref().to_ascii_lowercase()).or_default();
            if !list.contains(&(owner, ip)) {
                list.push((owner, ip));
            }
        }
    }

    /// A removed network's bridge: its server and names go.
    pub fn forget_bridge(&mut self, bridge: &str) {
        self.servers.retain(|s| s.container.is_some() || !s.bridges.iter().any(|b| b == bridge));
        for s in &mut self.servers {
            s.bridges.retain(|b| b != bridge);
        }
        self.names.remove(bridge);
        self.names6.remove(bridge);
        self.forget_pending();
    }

    /// Where queries from `client` that no container name answers go;
    /// empty means the VM's resolvers.
    pub fn set_upstream(&mut self, client: [u8; 4], servers: Vec<[u8; 4]>) {
        if servers.is_empty() {
            self.clients.remove(&client);
        } else {
            self.clients.insert(client, servers);
        }
    }

    pub fn unregister(&mut self, bridge: &str, ip: [u8; 4]) {
        self.clients.remove(&ip);
        if let Some(table) = self.names.get_mut(bridge) {
            for list in table.values_mut() {
                list.retain(|a| *a != ip);
            }
            table.retain(|_, list| !list.is_empty());
        }
        if let Some(table) = self.names6.get_mut(bridge) {
            for list in table.values_mut() {
                list.retain(|(owner, _)| *owner != ip);
            }
            table.retain(|_, list| !list.is_empty());
        }
    }

    /// Descriptors to poll: each server's, plus each forwarded query's.
    pub fn fds(&self) -> impl Iterator<Item = RawFd> + '_ {
        self.servers.iter().map(|s| s.sock.as_raw_fd()).chain(self.pending.iter().map(|p| p.sock.as_raw_fd()))
    }

    pub fn owns(&self, fd: RawFd) -> bool {
        self.fds().any(|f| f == fd)
    }

    pub fn on_readable(&mut self, fd: RawFd, upstream: &[[u8; 4]]) {
        let mut buf = [0u8; 1500];
        if let Some(at) = self.pending.iter().position(|p| p.sock.as_raw_fd() == fd) {
            // A forwarded answer: hand it back to whoever asked.
            loop {
                // SAFETY: receiving into a buffer we own.
                let n = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
                if n < 12 {
                    break;
                }
                let msg = &mut buf[..n as usize];
                let p = &self.pending[at];
                if u16::from_be_bytes([msg[0], msg[1]]) == p.our && response_matches(msg, p) {
                    let p = self.pending.swap_remove(at);
                    msg[0..2].copy_from_slice(&p.id.to_be_bytes());
                    if let Some(s) = self.servers.iter().find(|s| s.sock.as_raw_fd() == p.server) {
                        send_to(s.sock.as_raw_fd(), msg, &p.client);
                    }
                    break;
                }
            }
            return;
        }
        let Some(server) = self.servers.iter().position(|s| s.sock.as_raw_fd() == fd) else {
            return;
        };
        loop {
            // SAFETY: sockaddr_in is plain data; recvfrom fills it in.
            let mut from: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            let mut len = std::mem::size_of::<libc::sockaddr_in>() as u32;
            // SAFETY: receiving into buffers we own.
            let n = unsafe { libc::recvfrom(fd, buf.as_mut_ptr().cast(), buf.len(), 0, (&mut from as *mut libc::sockaddr_in).cast(), &mut len) };
            if n < 12 {
                break;
            }
            let query = &buf[..n as usize];
            match self.answer(server, query) {
                Some(reply) => send_to(fd, &reply, &from),
                None => self.forward(server, query, from, upstream),
            }
        }
    }

    /// Answers a query for a name on this network, or None to forward it.
    fn answer(&self, server: usize, q: &[u8]) -> Option<Vec<u8>> {
        let (name, qtype, end) = parse_question(q)?;
        let host = vec![super::net::HOST_ADDR];
        // The first of the server's networks that has the name answers.
        let bridge = self.servers[server].bridges.iter().find(|b| {
            self.names.get(*b).is_some_and(|t| t.contains_key(&name)) || self.names6.get(*b).is_some_and(|t| t.contains_key(&name))
        });
        let addrs = bridge.and_then(|b| self.names.get(b)).and_then(|t| t.get(&name));
        let addrs6 = bridge.and_then(|b| self.names6.get(b)).and_then(|t| t.get(&name));
        // The host's names answer A with the host's address, and AAAA with
        // no records: the host relay is IPv4 only.
        let addrs = match addrs {
            None if addrs6.is_none() && HOST_NAMES.contains(&name.as_str()) => Some(&host),
            a => a,
        };
        // A container name with no address of the asked type is NODATA, not
        // forwarded: the name is this network's.
        if addrs.is_none() && addrs6.is_none() {
            return None;
        }
        let a: &[[u8; 4]] = if qtype == 1 || qtype == 255 { addrs.map_or(&[], Vec::as_slice) } else { &[] };
        let aaaa: &[([u8; 4], [u8; 16])] = if qtype == 28 || qtype == 255 { addrs6.map_or(&[], Vec::as_slice) } else { &[] };
        let mut r = Vec::with_capacity(end + 16 * a.len() + 28 * aaaa.len());
        r.extend_from_slice(&q[0..2]); // id
        r.extend_from_slice(&[0x81, 0x80]); // response, recursion desired + available
        r.extend_from_slice(&1u16.to_be_bytes());
        r.extend_from_slice(&((a.len() + aaaa.len()) as u16).to_be_bytes());
        r.extend_from_slice(&[0, 0, 0, 0]);
        r.extend_from_slice(&q[12..end]);
        for ip in a {
            r.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1]);
            r.extend_from_slice(&TTL.to_be_bytes());
            r.extend_from_slice(&[0, 4]);
            r.extend_from_slice(ip);
        }
        for (_, ip) in aaaa {
            r.extend_from_slice(&[0xc0, 0x0c, 0, 28, 0, 1]);
            r.extend_from_slice(&TTL.to_be_bytes());
            r.extend_from_slice(&[0, 16]);
            r.extend_from_slice(ip);
        }
        Some(r)
    }

    fn forward(&mut self, server: usize, q: &[u8], client: libc::sockaddr_in, upstream: &[[u8; 4]]) {
        let now = Instant::now();
        self.pending.retain(|p| now.duration_since(p.sent) < FORWARD_TIMEOUT);
        let key = self.servers[server].container.map_or(client.sin_addr.s_addr.to_ne_bytes(), |(_, owner)| owner);
        let own = self.clients.get(&key).and_then(|v| v.first());
        let Some(dest) = own.or(upstream.first()).copied() else { return };
        if self.pending.len() >= MAX_PENDING {
            return;
        }
        let Some((_, _, end)) = parse_question(q) else {
            return;
        };
        let Ok(sock) = udp_socket() else { return };
        let to = sockaddr(dest, 53);
        // Connected, so it only receives datagrams from this resolver.
        // SAFETY: connecting to a valid sockaddr_in.
        if unsafe { libc::connect(sock.as_raw_fd(), (&to as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as u32) } != 0 {
            return;
        }
        let Some(our) = random_id() else { return };
        let mut msg = q.to_vec();
        let id = u16::from_be_bytes([msg[0], msg[1]]);
        msg[0..2].copy_from_slice(&our.to_be_bytes());
        // SAFETY: sending a buffer we own on a connected socket.
        if unsafe { libc::send(sock.as_raw_fd(), msg.as_ptr().cast(), msg.len(), 0) } < 0 {
            return;
        }
        self.pending.push(Pending { sock, our, server: self.servers[server].sock.as_raw_fd(), client, id, question: q[12..end].to_vec(), sent: now });
    }
}

/// A UDP socket bound to `ip`:53.
fn bound(ip: [u8; 4]) -> io::Result<OwnedFd> {
    let sock = udp_socket()?;
    let addr = sockaddr(ip, 53);
    // SAFETY: binding to a valid sockaddr_in.
    sys::check(
        unsafe { libc::bind(sock.as_raw_fd(), (&addr as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as u32) } as _,
        "dns bind",
    )?;
    Ok(sock)
}

fn response_matches(msg: &[u8], pending: &Pending) -> bool {
    msg.get(2).is_some_and(|flags| flags & 0x80 != 0) && msg.get(12..12 + pending.question.len()).is_some_and(|question| question == pending.question)
}

fn random_id() -> Option<u16> {
    let mut bytes = [0u8; 2];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut bytes).ok()?;
    Some(u16::from_ne_bytes(bytes))
}

/// The first question: (lowercase name without the trailing dot, qtype,
/// offset just past the question).
fn parse_question(q: &[u8]) -> Option<(String, u16, usize)> {
    if u16::from_be_bytes([q[4], q[5]]) != 1 {
        return None;
    }
    let mut i = 12;
    let mut name = String::new();
    loop {
        let len = *q.get(i)? as usize;
        if len == 0 {
            i += 1;
            break;
        }
        if len & 0xc0 != 0 {
            return None; // compression in a question: not from a sane client
        }
        let label = q.get(i + 1..i + 1 + len)?;
        if !name.is_empty() {
            name.push('.');
        }
        name.push_str(&String::from_utf8_lossy(label).to_ascii_lowercase());
        i += 1 + len;
    }
    let qtype = u16::from_be_bytes([*q.get(i)?, *q.get(i + 1)?]);
    Some((name, qtype, i + 4))
}

#[cfg(test)]
mod tests {
    use super::{Dns, Pending, Server, response_matches, udp_socket};

    #[test]
    fn host_names_answer_on_every_network() {
        let mut dns = Dns::default();
        dns.servers.push(Server { bridges: vec!["kiri1".into()], sock: udp_socket().unwrap(), container: None });
        let q = b"\x12\x34\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x04host\x06docker\x08internal\x00\x00\x01\x00\x01";
        let r = dns.answer(0, q).expect("answered, not forwarded");
        assert_eq!(&r[6..8], &[0, 1], "one answer");
        assert_eq!(&r[r.len() - 4..], &super::super::net::HOST_ADDR);
        let other = b"\x12\x34\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01";
        assert!(dns.answer(0, other).is_none(), "anything else is forwarded");
        // AAAA: answered with no records (NODATA), not forwarded upstream.
        let q6 = query("host.kiri.internal", 28);
        let r = dns.answer(0, &q6).expect("AAAA answered, not forwarded");
        assert_eq!((r[3] & 0x0f, answers(&r, q6.len()).len()), (0, 0));
        let q = query("gateway.docker.internal", 255);
        assert_eq!(answers(&dns.answer(0, &q).unwrap(), q.len()), [(1, super::super::net::HOST_ADDR.to_vec())]);
    }

    #[test]
    fn a_containers_server_answers_for_each_of_its_networks() {
        let mut dns = Dns::default();
        dns.servers.push(Server { bridges: vec!["kiri1".into(), "kiri2".into()], sock: udp_socket().unwrap(), container: Some((42, [10, 5, 0, 2])) });
        dns.register("kiri1", ["web"].iter(), [10, 5, 0, 3]);
        dns.register("kiri2", ["db"].iter(), [10, 6, 0, 3]);
        let q = query("db", 1);
        assert_eq!(answers(&dns.answer(0, &q).unwrap(), q.len()), [(1, vec![10, 6, 0, 3])]);
        let q = query("web", 1);
        assert_eq!(answers(&dns.answer(0, &q).unwrap(), q.len()), [(1, vec![10, 5, 0, 3])]);
        dns.leave(42, "kiri2");
        assert!(dns.answer(0, &query("db", 1)).is_none(), "gone with the network");
        dns.drop_container(42);
        assert!(dns.servers.is_empty());
    }

    #[test]
    fn every_forwarded_query_has_its_own_port_and_its_answer_goes_back() {
        use std::os::fd::AsRawFd;
        let bind = |ip: [u8; 4], port: u16| {
            let s = udp_socket().unwrap();
            let a = super::sockaddr(ip, port);
            assert_eq!(unsafe { libc::bind(s.as_raw_fd(), (&a as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as u32) }, 0, "bind");
            s
        };
        let name = |fd: i32| {
            let mut a: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            let mut len = std::mem::size_of::<libc::sockaddr_in>() as u32;
            unsafe { libc::getsockname(fd, (&mut a as *mut libc::sockaddr_in).cast(), &mut len) };
            a
        };
        let recv = |fd: i32| {
            let mut buf = [0u8; 512];
            let mut from: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            let mut len = std::mem::size_of::<libc::sockaddr_in>() as u32;
            for _ in 0..100 {
                let n = unsafe { libc::recvfrom(fd, buf.as_mut_ptr().cast(), buf.len(), 0, (&mut from as *mut libc::sockaddr_in).cast(), &mut len) };
                if n > 0 {
                    return (buf[..n as usize].to_vec(), from);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("nothing arrived");
        };
        // A resolver at 127.0.0.2:53, a server for the containers, a client.
        let resolver = bind([127, 0, 0, 2], 53);
        let server = bind([127, 0, 0, 3], 0);
        let client = bind([127, 0, 0, 1], 0);
        let mut dns = Dns::default();
        dns.servers.push(Server { bridges: vec!["kiri1".into()], sock: server, container: None });
        let server_fd = dns.servers[0].sock.as_raw_fd();
        let to = name(server_fd);
        let mut ports = Vec::new();
        for id in [0x1111u16, 0x2222] {
            let mut q = query("example.com", 1);
            q[0..2].copy_from_slice(&id.to_be_bytes());
            super::send_to(client.as_raw_fd(), &q, &to);
            std::thread::sleep(std::time::Duration::from_millis(20));
            dns.on_readable(server_fd, &[[127, 0, 0, 2]]);
            let (got, from) = recv(resolver.as_raw_fd());
            ports.push(from.sin_port);
            // The resolver answers; its answer goes back with the client's ID.
            let mut reply = got.clone();
            reply[2] |= 0x80;
            super::send_to(resolver.as_raw_fd(), &reply, &from);
            std::thread::sleep(std::time::Duration::from_millis(20));
            let fd = dns.pending.iter().find(|p| p.our == u16::from_be_bytes([got[0], got[1]])).expect("in flight").sock.as_raw_fd();
            dns.on_readable(fd, &[[127, 0, 0, 2]]);
            let (answer, _) = recv(client.as_raw_fd());
            assert_eq!(u16::from_be_bytes([answer[0], answer[1]]), id);
        }
        assert_ne!(ports[0], ports[1], "a fresh source port per query");
        assert!(dns.pending.is_empty(), "answered queries close their sockets");
    }

    #[test]
    fn own_servers_go_with_the_container() {
        let mut dns = Dns::default();
        dns.set_upstream([10, 5, 0, 2], vec![[8, 8, 8, 8]]);
        assert_eq!(dns.clients.get(&[10, 5, 0, 2]), Some(&vec![[8, 8, 8, 8]]));
        dns.unregister("kiri1", [10, 5, 0, 2]);
        assert!(dns.clients.is_empty());
        dns.set_upstream([10, 5, 0, 3], vec![]);
        assert!(dns.clients.is_empty());
    }
    use std::time::Instant;

    /// A query for `name` of `qtype`, as a resolver sends it.
    fn query(name: &str, qtype: u16) -> Vec<u8> {
        let mut q = vec![0xab, 0xcd, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in name.split('.') {
            q.push(label.len() as u8);
            q.extend_from_slice(label.as_bytes());
        }
        q.push(0);
        q.extend_from_slice(&qtype.to_be_bytes());
        q.extend_from_slice(&[0, 1]);
        q
    }

    /// (type, data) of each answer.
    fn answers(r: &[u8], qlen: usize) -> Vec<(u16, Vec<u8>)> {
        let count = u16::from_be_bytes([r[6], r[7]]) as usize;
        let mut i = qlen;
        let mut out = Vec::new();
        for _ in 0..count {
            let ty = u16::from_be_bytes([r[i + 2], r[i + 3]]);
            let len = u16::from_be_bytes([r[i + 10], r[i + 11]]) as usize;
            out.push((ty, r[i + 12..i + 12 + len].to_vec()));
            i += 12 + len;
        }
        out
    }

    #[test]
    fn names_answer_a_and_aaaa() {
        let mut dns = Dns::default();
        // A server needs a socket; any UDP socket stands in for the bridge's.
        dns.servers.push(super::Server { bridges: vec!["kiri1".into()], sock: super::udp_socket().unwrap(), container: None });
        dns.register("kiri1", ["web", "Alias"].iter(), [10, 5, 0, 2]);
        let v6 = [0xfd, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
        dns.register6("kiri1", ["web", "Alias"].iter(), [10, 5, 0, 2], v6);
        dns.register("kiri1", ["old"].iter(), [10, 5, 0, 3]);

        let q = query("web", 1);
        assert_eq!(answers(&dns.answer(0, &q).unwrap(), q.len()), [(1, vec![10, 5, 0, 2])]);
        let q = query("alias", 28);
        assert_eq!(answers(&dns.answer(0, &q).unwrap(), q.len()), [(28, v6.to_vec())]);
        let q = query("web", 255);
        assert_eq!(answers(&dns.answer(0, &q).unwrap(), q.len()).len(), 2);
        // An IPv4-only container: AAAA is answered, empty (NODATA).
        let q = query("old", 28);
        let r = dns.answer(0, &q).unwrap();
        assert_eq!((r[3] & 0x0f, answers(&r, q.len()).len()), (0, 0));
        // Not a container: forwarded.
        assert!(dns.answer(0, &query("example.com", 28)).is_none());
        // Gone with the container's IPv4 address.
        dns.unregister("kiri1", [10, 5, 0, 2]);
        assert!(dns.answer(0, &query("web", 28)).is_none());
        dns.forget_bridge("kiri1");
        assert!(dns.servers.is_empty() && dns.names.is_empty() && dns.names6.is_empty());
    }

    fn pending() -> Pending {
        Pending {
            sock: udp_socket().unwrap(),
            our: 1,
            server: -1,
            client: unsafe { std::mem::zeroed() },
            id: 7,
            question: b"\x03www\x07example\x03com\x00\x00\x01\x00\x01".to_vec(),
            sent: Instant::now(),
        }
    }

    #[test]
    fn response_must_match_the_whole_question() {
        let p = pending();
        let mut response = vec![0x12, 0x34, 0x81, 0x80, 0, 1, 0, 1, 0, 0, 0, 0];
        response.extend_from_slice(&p.question);
        assert!(response_matches(&response, &p));

        let mut wrong_name = response.clone();
        wrong_name[12] = 3;
        wrong_name[13] = b'x';
        assert!(!response_matches(&wrong_name, &p));

        let mut wrong_type = response.clone();
        *wrong_type.last_mut().unwrap() = 6;
        assert!(!response_matches(&wrong_type, &p));

        response[2] = 0x01;
        assert!(!response_matches(&response, &p));
    }
}
