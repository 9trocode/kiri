//! Boots Kiri's Linux guest with Apple's Virtualization.framework.
//!
//! Every Virtualization.framework object lives on one private serial dispatch
//! queue and is only touched there; the rest of Kiri sees plain Rust types and
//! events. The guest dials the host over vsock when it is ready, so start-up
//! needs no polling.


use std::cell::RefCell;
use std::io;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::runtime::Bool;
use objc2::{AllocAnyThread, DefinedClass, Message, define_class, msg_send};
use objc2_foundation::{NSArray, NSError, NSFileHandle, NSObject, NSObjectProtocol, NSString, NSURL};
use objc2_virtualization::*;

#[derive(Debug)]
pub struct Config {
    pub cpus: usize,
    pub memory_mib: u64,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub cmdline: String,
    /// Where the guest console goes. `None` discards it.
    pub console: Option<OwnedFd>,
    /// vsock port the guest agent dials.
    pub agent_port: u32,
    /// Raw disk image for the guest's data (layers and containers): `/dev/vda`.
    pub disk: Option<PathBuf>,
    /// Share a host directory with the guest over virtiofs: (directory, tag).
    pub share: Option<(PathBuf, String)>,
    /// Share Rosetta with the guest over virtiofs under this tag, so it can
    /// run x86-64 Linux binaries. Ignored if Rosetta is not installed.
    pub rosetta_tag: Option<String>,
    /// Attach a NAT network card (`eth0` in the guest) with this MAC address
    /// (`aa:bb:cc:dd:ee:ff`). A stable address keeps the DHCP lease stable.
    pub nat_mac: Option<String>,
}

#[derive(Debug)]
pub enum Event {
    /// The guest agent connected. The fd is a stream socket owned by the caller.
    GuestConnected(OwnedFd),
    /// The VM stopped, with the reason if it was an error.
    Stopped(Option<String>),
}

#[derive(Debug)]
pub struct Vm {
    queue: DispatchRetained<DispatchQueue>,
    inner: Option<OnQueue<Inner>>,
    events: Receiver<Event>,
}

/// Virtualization.framework objects, kept alive for as long as the VM runs.
/// Delegates are weak references in Objective-C, so they are held here too.
struct Inner {
    vm: Retained<VZVirtualMachine>,
    _vm_delegate: Retained<VmDelegate>,
    _listener: Retained<VZVirtioSocketListener>,
    _listener_delegate: Retained<ListenerDelegate>,
    /// Listeners added with `Vm::listen`, kept alive with the VM.
    extra: RefCell<Vec<(Retained<VZVirtioSocketListener>, Retained<PortDelegate>)>>,
}

/// Wraps a value that may only be used on the VM's queue.
///
/// Objective-C objects are not `Send`, but the queue serialises all access, so
/// moving them across threads is sound as long as they are only touched from
/// inside closures that run on that queue — which is the only way this module
/// ever reaches them.
struct OnQueue<T>(T);
// SAFETY: see the type's documentation; values are only touched on the queue.
unsafe impl<T> Send for OnQueue<T> {}

impl std::fmt::Debug for OnQueue<Inner> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OnQueue<Inner>")
    }
}

impl Vm {
    pub fn is_supported() -> bool {
        // SAFETY: a class property with no preconditions.
        unsafe { VZVirtualMachine::isSupported() }
    }

    /// Builds, validates and starts the VM. Returns once Virtualization.framework
    /// reports the VM running; the guest connects later as an [`Event`].
    pub fn start(cfg: Config) -> io::Result<Vm> {
        let queue = DispatchQueue::new("dev.kiri.vm", DispatchQueueAttr::SERIAL);
        let (tx, events) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::sync_channel::<Result<(), String>>(1);

        let mut built: Option<io::Result<OnQueue<Inner>>> = None;
        let slot = OnQueue(&mut built);
        let q = queue.clone();
        queue.exec_sync(move || {
            let slot = slot;
            *slot.0 = Some(build(cfg, &q, tx).map(OnQueue));
            if let Some(Ok(inner)) = slot.0.as_ref() {
                let started_tx = started_tx.clone();
                let done = RcBlock::new(move |err: *mut NSError| {
                    // SAFETY: the framework passes either null or a valid NSError.
                    let res = unsafe { err.as_ref() }.map_or(Ok(()), |e| Err(describe(e)));
                    let _ = started_tx.try_send(res);
                });
                // SAFETY: called on the VM's queue, as the framework requires.
                unsafe { inner.0.vm.startWithCompletionHandler(&done) };
            }
        });
        let inner = built.expect("build ran synchronously")?;
        let vm = Vm { queue, inner: Some(inner), events };
        match started_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => Ok(vm),
            Ok(Err(e)) => Err(io::Error::other(format!("VM failed to start: {e}"))),
            Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "VM did not start within 30 s")),
        }
    }

    /// Waits up to `timeout` for the next event.
    pub fn next_event(&self, timeout: Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Stops the VM immediately, like pulling the power cord. Prefer asking the
    /// guest to power off; this is the fallback.
    pub fn force_stop(&self) {
        let Some(inner) = self.inner.as_ref() else { return };
        let inner = OnQueue(&inner.0);
        let (tx, rx) = mpsc::sync_channel::<()>(1);
        self.queue.exec_sync(move || {
            let inner = inner;
            // SAFETY: on the VM's queue.
            unsafe {
                if inner.0.vm.canStop() {
                    let done = RcBlock::new(move |_err: *mut NSError| {
                        let _ = tx.try_send(());
                    });
                    inner.0.vm.stopWithCompletionHandler(&done);
                } else {
                    let _ = tx.try_send(());
                }
            }
        });
        let _ = rx.recv_timeout(Duration::from_secs(10));
    }

    pub fn is_running(&self) -> bool {
        let Some(inner) = self.inner.as_ref() else { return false };
        let inner = OnQueue(&inner.0);
        let mut running = false;
        let out = OnQueue(&mut running);
        self.queue.exec_sync(move || {
            let (inner, out) = (inner, out);
            // SAFETY: on the VM's queue.
            *out.0 = unsafe { inner.0.vm.state() } == VZVirtualMachineState::Running;
        });
        running
    }
}

/// A host-initiated vsock stream into the guest. The framework closes the
/// socket when its connection object is released, so the object lives as
/// long as this, and is released on the VM's queue when this drops.
pub struct VsockStream {
    pub fd: OwnedFd,
    conn: Option<OnQueue<Retained<VZVirtioSocketConnection>>>,
    queue: DispatchRetained<DispatchQueue>,
}

impl std::fmt::Debug for VsockStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VsockStream").field("fd", &self.fd).finish()
    }
}

impl Drop for VsockStream {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.queue.exec_async(move || drop(conn));
        }
    }
}

/// What `Vm::listen` calls for each guest connection. It runs on the VM's
/// queue, so it should hand the stream to a thread and return.
pub type Handler = Box<dyn Fn(VsockStream) + Send + Sync>;

impl Vm {
    /// Asks the guest (through the balloon) to run in `bytes` of memory. The
    /// guest gives the rest back to macOS. Returns the previous target.
    pub fn set_memory_target(&self, bytes: u64) -> Option<u64> {
        let inner = OnQueue(&self.inner.as_ref()?.0);
        let mut previous = None;
        let out = OnQueue(&mut previous);
        self.queue.exec_sync(move || {
            let (inner, out) = (inner, out);
            // SAFETY: on the VM's queue, as the framework requires.
            unsafe {
                let Some(dev) = inner.0.vm.memoryBalloonDevices().firstObject() else { return };
                let Ok(dev) = Retained::downcast::<VZVirtioTraditionalMemoryBalloonDevice>(dev) else { return };
                *out.0 = Some(dev.targetVirtualMachineMemorySize());
                dev.setTargetVirtualMachineMemorySize(bytes);
            }
        });
        previous
    }

    /// Accepts guest-initiated connections on `port`, passing each one to
    /// `handler` as a stream that releases its connection when dropped.
    pub fn listen(&self, port: u32, handler: Handler) -> io::Result<()> {
        let Some(inner) = self.inner.as_ref() else { return Err(io::Error::other("VM is gone")) };
        let inner = OnQueue(&inner.0);
        let handler = OnQueue(handler);
        let queue = self.queue.clone();
        let mut result = Err(io::Error::other("vsock device missing"));
        let out = OnQueue(&mut result);
        self.queue.exec_sync(move || {
            let (inner, handler, out) = (inner, handler, out);
            // SAFETY: on the VM's queue, as the framework requires.
            unsafe {
                let device = inner.0.vm.socketDevices().firstObject();
                let Some(device) = device.and_then(|d| Retained::downcast::<VZVirtioSocketDevice>(d).ok()) else { return };
                let listener = VZVirtioSocketListener::new();
                let delegate = PortDelegate::new(handler.0, queue);
                listener.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
                device.setSocketListener_forPort(&listener, port);
                inner.0.extra.borrow_mut().push((listener, delegate));
                *out.0 = Ok(());
            }
        });
        result
    }

    /// Opens a stream to a port the guest listens on.
    pub fn connect(&self, port: u32) -> io::Result<VsockStream> {
        self.connector().connect(port)
    }

    /// What opens streams to the guest's ports from any thread, while the
    /// `Vm` itself is kept elsewhere: a connect waits on the guest, so
    /// holding the `Vm` meanwhile would make connects take turns.
    pub fn connector(&self) -> Connector {
        // Retaining is thread-safe; the VM is otherwise touched only on its queue.
        Connector { queue: self.queue.clone(), vm: self.inner.as_ref().map(|i| OnQueue(i.0.vm.clone())) }
    }
}

/// Opens streams to a VM's guest ports (`Vm::connector`).
pub struct Connector {
    queue: DispatchRetained<DispatchQueue>,
    vm: Option<OnQueue<Retained<VZVirtualMachine>>>,
}

// SAFETY: the VM inside is only touched in closures run on its queue.
unsafe impl Sync for Connector {}

impl std::fmt::Debug for Connector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Connector")
    }
}

impl Connector {
    /// Opens a stream to a port the guest listens on.
    pub fn connect(&self, port: u32) -> io::Result<VsockStream> {
        let Some(vm) = self.vm.as_ref() else { return Err(io::Error::other("VM is gone")) };
        let vm = OnQueue(&vm.0);
        let (tx, rx) = mpsc::sync_channel::<Result<OnQueue<Retained<VZVirtioSocketConnection>>, String>>(1);
        self.queue.exec_sync(move || {
            let vm = vm;
            // SAFETY: on the VM's queue, as the framework requires.
            unsafe {
                let device = vm.0.socketDevices().firstObject();
                let Some(device) = device.and_then(|d| Retained::downcast::<VZVirtioSocketDevice>(d).ok()) else {
                    let _ = tx.try_send(Err("VM has no vsock device".into()));
                    return;
                };
                let done = RcBlock::new(move |conn: *mut VZVirtioSocketConnection, err: *mut NSError| {
                    let result = match (conn.as_ref(), err.as_ref()) {
                        (Some(c), _) => Ok(OnQueue(c.retain())),
                        (None, Some(e)) => Err(describe(e)),
                        (None, None) => Err("vsock connect failed".into()),
                    };
                    let _ = tx.try_send(result);
                });
                device.connectToPort_completionHandler(port, &done);
            }
        });
        let conn = match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(io::Error::new(io::ErrorKind::ConnectionRefused, e)),
            Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "vsock connect timed out")),
        };
        // SAFETY: the connection owns a valid descriptor while it is alive;
        // the duplicate is ours and independent of it.
        let fd = unsafe { libc::dup(conn.0.fileDescriptor()) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is a fresh descriptor that nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(VsockStream { fd, conn: Some(conn), queue: self.queue.clone() })
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        if self.is_running() {
            self.force_stop();
        }
        // Release every framework object on the queue that owns it.
        if let Some(inner) = self.inner.take() {
            self.queue.exec_sync(move || drop(inner));
        }
    }
}

fn describe(e: &NSError) -> String {
    e.localizedDescription().to_string()
}

/// Whether macOS has Rosetta for Linux installed
/// (`softwareupdate --install-rosetta` adds it).
#[cfg(target_arch = "aarch64")]
pub fn rosetta_installed() -> bool {
    // SAFETY: a class method with no arguments.
    let availability = unsafe { VZLinuxRosettaDirectoryShare::availability() };
    availability == VZLinuxRosettaAvailability::Installed
}

/// An Intel Mac has no Rosetta for Linux (the framework only has it on
/// Apple Silicon); its guest is x86-64, so amd64 images run natively.
#[cfg(not(target_arch = "aarch64"))]
pub fn rosetta_installed() -> bool {
    false
}

fn url(path: &std::path::Path) -> io::Result<Retained<NSURL>> {
    if !path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", path.display())));
    }
    Ok(NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy())))
}

/// Runs on the VM's queue.
fn build(cfg: Config, queue: &DispatchQueue, events: Sender<Event>) -> io::Result<Inner> {
    // SAFETY: plain configuration calls on objects created here, on the queue
    // that will own the VM.
    unsafe {
        let kernel = url(&cfg.kernel)?;
        let initrd = url(&cfg.initrd)?;
        let boot = VZLinuxBootLoader::initWithKernelURL(VZLinuxBootLoader::alloc(), &kernel);
        boot.setInitialRamdiskURL(Some(&initrd));
        boot.setCommandLine(&NSString::from_str(&cfg.cmdline));

        let conf = VZVirtualMachineConfiguration::new();
        conf.setBootLoader(Some(&boot));
        conf.setPlatform(&VZGenericPlatformConfiguration::new());
        conf.setCPUCount(cfg.cpus);
        conf.setMemorySize(cfg.memory_mib * 1024 * 1024);

        let console = VZVirtioConsoleDeviceSerialPortConfiguration::new();
        let sink = cfg.console.map(|fd| NSFileHandle::initWithFileDescriptor_closeOnDealloc(
                NSFileHandle::alloc(),
                fd.into_raw_fd(),
                true,
            ));
        let attachment = VZFileHandleSerialPortAttachment::initWithFileHandleForReading_fileHandleForWriting(
            VZFileHandleSerialPortAttachment::alloc(),
            None,
            sink.as_deref(),
        );
        console.setAttachment(Some(&attachment));
        conf.setSerialPorts(&NSArray::from_retained_slice(&[Retained::into_super(console)]));

        conf.setSocketDevices(&NSArray::from_retained_slice(&[Retained::into_super(
            VZVirtioSocketDeviceConfiguration::new(),
        )]));
        conf.setEntropyDevices(&NSArray::from_retained_slice(&[Retained::into_super(
            VZVirtioEntropyDeviceConfiguration::new(),
        )]));
        // Lets the host take memory back from an idle guest.
        conf.setMemoryBalloonDevices(&NSArray::from_retained_slice(&[Retained::into_super(
            VZVirtioTraditionalMemoryBalloonDeviceConfiguration::new(),
        )]));

        if let Some(disk) = &cfg.disk {
            let disk_url = url(disk)?;
            // Cached with fsync: Apple's "full" sync is much slower, and the
            // disk only holds data the host can always send again.
            let attachment = VZDiskImageStorageDeviceAttachment::initWithURL_readOnly_cachingMode_synchronizationMode_error(
                VZDiskImageStorageDeviceAttachment::alloc(),
                &disk_url,
                false,
                VZDiskImageCachingMode::Cached,
                VZDiskImageSynchronizationMode::Fsync,
            )
            .map_err(|e| io::Error::other(format!("data disk: {}", describe(&e))))?;
            let block = VZVirtioBlockDeviceConfiguration::initWithAttachment(
                VZVirtioBlockDeviceConfiguration::alloc(),
                &attachment,
            );
            conf.setStorageDevices(&NSArray::from_retained_slice(&[Retained::into_super(block)]));
        }

        let mut shares = Vec::new();
        if let Some((dir, tag)) = &cfg.share {
            let dir_url = url(dir)?;
            let shared = VZSharedDirectory::initWithURL_readOnly(VZSharedDirectory::alloc(), &dir_url, false);
            let share = VZSingleDirectoryShare::initWithDirectory(VZSingleDirectoryShare::alloc(), &shared);
            let fs = VZVirtioFileSystemDeviceConfiguration::initWithTag(VZVirtioFileSystemDeviceConfiguration::alloc(), &NSString::from_str(tag));
            fs.setShare(Some(&share));
            shares.push(Retained::into_super(fs));
        }
        // Rosetta for Linux exists only on Apple Silicon.
        #[cfg(target_arch = "aarch64")]
        if let Some(tag) = cfg.rosetta_tag.as_ref().filter(|_| rosetta_installed()) {
            match VZLinuxRosettaDirectoryShare::initWithError(VZLinuxRosettaDirectoryShare::alloc()) {
                Ok(rosetta) => {
                    let fs = VZVirtioFileSystemDeviceConfiguration::initWithTag(VZVirtioFileSystemDeviceConfiguration::alloc(), &NSString::from_str(tag));
                    fs.setShare(Some(&rosetta));
                    shares.push(Retained::into_super(fs));
                }
                Err(e) => eprintln!("kiri-vmm: Rosetta unavailable: {}", describe(&e)),
            }
        }
        if !shares.is_empty() {
            conf.setDirectorySharingDevices(&NSArray::from_retained_slice(&shares));
        }

        if let Some(mac) = &cfg.nat_mac {
            let net = VZVirtioNetworkDeviceConfiguration::new();
            net.setAttachment(Some(&VZNATNetworkDeviceAttachment::new()));
            let parsed = VZMACAddress::initWithString(VZMACAddress::alloc(), &NSString::from_str(mac))
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("bad MAC address {mac}")))?;
            net.setMACAddress(&parsed);
            conf.setNetworkDevices(&NSArray::from_retained_slice(&[Retained::into_super(net)]));
        }

        conf.validateWithError().map_err(|e| io::Error::other(format!("invalid VM config: {}", describe(&e))))?;

        let vm = VZVirtualMachine::initWithConfiguration_queue(VZVirtualMachine::alloc(), &conf, queue);
        let vm_delegate = VmDelegate::new(events.clone());
        vm.setDelegate(Some(ProtocolObject::from_ref(&*vm_delegate)));

        let listener = VZVirtioSocketListener::new();
        let listener_delegate = ListenerDelegate::new(events);
        listener.setDelegate(Some(ProtocolObject::from_ref(&*listener_delegate)));
        let socket = vm.socketDevices().firstObject().ok_or_else(|| io::Error::other("VM has no vsock device"))?;
        let socket: Retained<VZVirtioSocketDevice> = Retained::downcast(socket)
            .map_err(|_| io::Error::other("vsock device has an unexpected type"))?;
        socket.setSocketListener_forPort(&listener, cfg.agent_port);

        Ok(Inner { vm, _vm_delegate: vm_delegate, _listener: listener, _listener_delegate: listener_delegate, extra: RefCell::new(Vec::new()) })
    }
}

struct VmDelegateIvars {
    events: Sender<Event>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this type has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "KiriVmDelegate"]
    #[ivars = VmDelegateIvars]
    struct VmDelegate;

    unsafe impl NSObjectProtocol for VmDelegate {}

    unsafe impl VZVirtualMachineDelegate for VmDelegate {
        #[unsafe(method(guestDidStopVirtualMachine:))]
        fn guest_did_stop(&self, _vm: &VZVirtualMachine) {
            let _ = self.ivars().events.send(Event::Stopped(None));
        }

        #[unsafe(method(virtualMachine:didStopWithError:))]
        fn did_stop_with_error(&self, _vm: &VZVirtualMachine, error: &NSError) {
            let _ = self.ivars().events.send(Event::Stopped(Some(describe(error))));
        }
    }
);

impl VmDelegate {
    fn new(events: Sender<Event>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(VmDelegateIvars { events });
        // SAFETY: NSObject's init on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

struct PortIvars {
    handler: Handler,
    queue: DispatchRetained<DispatchQueue>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this type has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "KiriVsockPortListener"]
    #[ivars = PortIvars]
    struct PortDelegate;

    unsafe impl NSObjectProtocol for PortDelegate {}

    unsafe impl VZVirtioSocketListenerDelegate for PortDelegate {
        #[unsafe(method(listener:shouldAcceptNewConnection:fromSocketDevice:))]
        fn should_accept(
            &self,
            _listener: &VZVirtioSocketListener,
            conn: &VZVirtioSocketConnection,
            _device: &VZVirtioSocketDevice,
        ) -> Bool {
            // SAFETY: the connection owns a valid descriptor while it is alive.
            let fd = unsafe { libc::dup(conn.fileDescriptor()) };
            if fd < 0 {
                return Bool::NO;
            }
            // SAFETY: `fd` is a fresh descriptor that nothing else owns.
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            let stream = VsockStream { fd, conn: Some(OnQueue(conn.retain())), queue: self.ivars().queue.clone() };
            (self.ivars().handler)(stream);
            Bool::YES
        }
    }
);

impl PortDelegate {
    fn new(handler: Handler, queue: DispatchRetained<DispatchQueue>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(PortIvars { handler, queue });
        // SAFETY: NSObject's init on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

struct ListenerIvars {
    events: Sender<Event>,
    /// The framework closes a connection's socket when the connection object
    /// is released, so the accepted connection is kept until the VM goes away.
    /// There is only ever one: kiri-init dials once at boot.
    connections: RefCell<Vec<Retained<VZVirtioSocketConnection>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this type has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "KiriVsockListener"]
    #[ivars = ListenerIvars]
    struct ListenerDelegate;

    unsafe impl NSObjectProtocol for ListenerDelegate {}

    unsafe impl VZVirtioSocketListenerDelegate for ListenerDelegate {
        #[unsafe(method(listener:shouldAcceptNewConnection:fromSocketDevice:))]
        fn should_accept(
            &self,
            _listener: &VZVirtioSocketListener,
            conn: &VZVirtioSocketConnection,
            _device: &VZVirtioSocketDevice,
        ) -> Bool {
            Bool::new(self.accept(conn))
        }
    }
);

impl ListenerDelegate {
    fn accept(&self, conn: &VZVirtioSocketConnection) -> bool {
        {
            let mut connections = self.ivars().connections.borrow_mut();
            // Once the control channel is up, anything else dialing this port
            // is a container that got hold of a vsock socket: refused, so the
            // framework closes it, rather than retained for the VM's lifetime.
            if !connections.is_empty() {
                return false;
            }
            // SAFETY: the connection owns a valid descriptor while it is alive;
            // the duplicate gives the caller its own independent reference.
            let fd = unsafe { libc::dup(conn.fileDescriptor()) };
            if fd < 0 {
                return false;
            }
            // SAFETY: `fd` is a fresh descriptor that nothing else owns.
            let owned = unsafe { OwnedFd::from_raw_fd(fd) };
            connections.push(conn.retain());
            self.ivars().events.send(Event::GuestConnected(owned)).is_ok()
        }
    }

    fn new(events: Sender<Event>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(ListenerIvars { events, connections: RefCell::new(Vec::new()) });
        // SAFETY: NSObject's init on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}
