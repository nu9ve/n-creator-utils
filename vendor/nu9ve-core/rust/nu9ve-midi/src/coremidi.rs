//! CoreMIDI for [`MidiSystem`], by direct FFI to the system frameworks: no third-party crate.
//!
//! A thread of its own creates the client and runs a `CFRunLoop`, where CoreMIDI delivers its
//! notifications (a source appears or goes away). It sleeps in the kernel until one comes: nothing polls.
//! Messages arrive on CoreMIDI's own receive thread. Both call the [`Sink`] directly.

use crate::port::{MidiSystem, PortEvent, Sink, split_channel_messages};
use std::collections::BTreeMap;
use std::ffi::{CString, c_char, c_void};
use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

type OSStatus = i32;
type MIDIObjectRef = u32;
type MIDIClientRef = MIDIObjectRef;
type MIDIPortRef = MIDIObjectRef;
type MIDIEndpointRef = MIDIObjectRef;
type CFStringRef = *const c_void;
type CFRunLoopRef = *mut c_void;

#[repr(C)]
struct MIDINotification {
    message_id: i32,
    message_size: u32,
}

type NotifyProc = extern "C" fn(*const MIDINotification, *mut c_void);
type ReadProc = extern "C" fn(*const c_void, *mut c_void, *mut c_void);

#[repr(C)]
struct MachTimebase {
    numer: u32,
    denom: u32,
}

#[link(name = "CoreMIDI", kind = "framework")]
unsafe extern "C" {
    static kMIDIPropertyDisplayName: CFStringRef;
    fn MIDIClientCreate(
        name: CFStringRef,
        notify: Option<NotifyProc>,
        refcon: *mut c_void,
        out: *mut MIDIClientRef,
    ) -> OSStatus;
    fn MIDIClientDispose(client: MIDIClientRef) -> OSStatus;
    fn MIDIInputPortCreate(
        client: MIDIClientRef,
        name: CFStringRef,
        read: ReadProc,
        refcon: *mut c_void,
        out: *mut MIDIPortRef,
    ) -> OSStatus;
    fn MIDIOutputPortCreate(
        client: MIDIClientRef,
        name: CFStringRef,
        out: *mut MIDIPortRef,
    ) -> OSStatus;
    fn MIDIPortConnectSource(
        port: MIDIPortRef,
        source: MIDIEndpointRef,
        refcon: *mut c_void,
    ) -> OSStatus;
    fn MIDIPortDisconnectSource(port: MIDIPortRef, source: MIDIEndpointRef) -> OSStatus;
    fn MIDIGetNumberOfSources() -> usize;
    fn MIDIGetSource(index: usize) -> MIDIEndpointRef;
    fn MIDIGetNumberOfDestinations() -> usize;
    fn MIDIGetDestination(index: usize) -> MIDIEndpointRef;
    fn MIDIObjectGetStringProperty(
        object: MIDIObjectRef,
        property: CFStringRef,
        out: *mut CFStringRef,
    ) -> OSStatus;
    fn MIDIPacketListInit(list: *mut c_void) -> *mut c_void;
    fn MIDIPacketListAdd(
        list: *mut c_void,
        size: usize,
        current: *mut c_void,
        time: u64,
        length: usize,
        data: *const u8,
    ) -> *mut c_void;
    fn MIDISend(port: MIDIPortRef, destination: MIDIEndpointRef, list: *const c_void) -> OSStatus;
    fn MIDISourceCreate(
        client: MIDIClientRef,
        name: CFStringRef,
        out: *mut MIDIEndpointRef,
    ) -> OSStatus;
    fn MIDIDestinationCreate(
        client: MIDIClientRef,
        name: CFStringRef,
        read: ReadProc,
        refcon: *mut c_void,
        out: *mut MIDIEndpointRef,
    ) -> OSStatus;
    fn MIDIReceived(source: MIDIEndpointRef, list: *const c_void) -> OSStatus;
    fn MIDIEndpointDispose(endpoint: MIDIEndpointRef) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        text: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(text: CFStringRef, buffer: *mut c_char, size: isize, encoding: u32)
    -> u8;
    fn CFRelease(object: *const c_void);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRun();
    fn CFRunLoopStop(run_loop: CFRunLoopRef);
}

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebase) -> i32;
}

const UTF8: u32 = 0x0800_0100;
const SETUP_CHANGED: i32 = 1;
const OBJECT_ADDED: i32 = 2;
const OBJECT_REMOVED: i32 = 3;
/// `kMIDIServerStartErr`: the MIDI server is still starting. Some runs report -304 for the same case.
const SERVER_STARTING: [OSStatus; 2] = [-10839, -304];

/// A CFString owned here, released on drop.
struct CfString(CFStringRef);

impl CfString {
    fn new(text: &str) -> Self {
        let c = CString::new(text.replace('\0', " ")).unwrap_or_default();
        // SAFETY: a NUL-terminated UTF-8 string; a null result is handled by the callers as "no name".
        Self(unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8) })
    }
}

impl Drop for CfString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: created by us with a +1 reference.
            unsafe { CFRelease(self.0) };
        }
    }
}

fn display_name(object: MIDIObjectRef) -> String {
    let mut value: CFStringRef = std::ptr::null();
    // SAFETY: CoreMIDI writes a +1 CFString or leaves it null.
    let status =
        unsafe { MIDIObjectGetStringProperty(object, kMIDIPropertyDisplayName, &mut value) };
    if status != 0 || value.is_null() {
        return String::new();
    }
    let owned = CfString(value);
    let mut buffer = vec![0 as c_char; 1024];
    // SAFETY: the buffer has room for 1024 bytes including the NUL.
    let ok = unsafe { CFStringGetCString(owned.0, buffer.as_mut_ptr(), 1024, UTF8) };
    if ok == 0 {
        return String::new();
    }
    // SAFETY: CFStringGetCString wrote a NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn sources() -> Vec<(String, MIDIEndpointRef)> {
    // SAFETY: plain queries of the MIDI setup.
    let count = unsafe { MIDIGetNumberOfSources() };
    (0..count)
        .map(|i| unsafe { MIDIGetSource(i) })
        .filter(|e| *e != 0)
        .map(|e| (display_name(e), e))
        .collect()
}

fn destination(name: &str) -> Option<MIDIEndpointRef> {
    // SAFETY: plain queries of the MIDI setup.
    let count = unsafe { MIDIGetNumberOfDestinations() };
    (0..count)
        .map(|i| unsafe { MIDIGetDestination(i) })
        .find(|e| *e != 0 && display_name(*e) == name)
}

/// Host ticks to nanoseconds.
pub fn host_ns(ticks: u64) -> u64 {
    let mut info = MachTimebase { numer: 0, denom: 0 };
    // SAFETY: fills the struct.
    unsafe { mach_timebase_info(&mut info) };
    if info.denom == 0 {
        return ticks;
    }
    (ticks as u128 * info.numer as u128 / info.denom as u128) as u64
}

/// The host clock now, in nanoseconds.
pub fn now_ns() -> u64 {
    // SAFETY: no arguments.
    host_ns(unsafe { mach_absolute_time() })
}

/// Walks a MIDIPacketList: (timestamp, bytes) of each packet. `MIDIPacket` is packed to 4 bytes:
/// timestamp at 0, length at 8, data at 10; the next packet is 4-aligned on arm64.
///
/// # Safety
/// `list` must point to a valid MIDIPacketList.
unsafe fn packets(list: *const c_void) -> Vec<(u64, Vec<u8>)> {
    let mut out = Vec::new();
    // SAFETY: the caller passes a valid list; every read stays inside its packets.
    unsafe {
        let count = std::ptr::read_unaligned(list as *const u32);
        let mut packet = (list as *const u8).add(4);
        for _ in 0..count {
            let stamp = std::ptr::read_unaligned(packet as *const u64);
            let length = std::ptr::read_unaligned(packet.add(8) as *const u16) as usize;
            let data = std::slice::from_raw_parts(packet.add(10), length).to_vec();
            out.push((stamp, data));
            let end = packet.add(10 + length) as usize;
            let next = if cfg!(target_arch = "aarch64") {
                (end + 3) & !3
            } else {
                end
            };
            packet = next as *const u8;
        }
    }
    out
}

/// Sends each message in its own packet list, so a SysEx stays whole.
fn send_to(
    port: MIDIPortRef,
    destination: MIDIEndpointRef,
    messages: &[Vec<u8>],
) -> io::Result<()> {
    const SIZE: usize = 1024;
    let mut buffer = vec![0u64; SIZE / 8];
    for message in messages {
        if message.is_empty() || message.len() > SIZE - 64 {
            continue;
        }
        let list = buffer.as_mut_ptr() as *mut c_void;
        // SAFETY: the buffer is SIZE bytes, 8-aligned; the message fits.
        let status = unsafe {
            let packet = MIDIPacketListInit(list);
            MIDIPacketListAdd(list, SIZE, packet, 0, message.len(), message.as_ptr());
            MIDISend(port, destination, list)
        };
        if status != 0 {
            return Err(io::Error::other(format!("MIDISend: {status}")));
        }
    }
    Ok(())
}

struct Open {
    source: MIDIEndpointRef,
    destination: Option<MIDIEndpointRef>,
    running: Option<u8>,
}

/// Shared with CoreMIDI's callbacks.
struct Shared {
    sink: Sink,
    known: Mutex<Vec<String>>,
    open: Mutex<BTreeMap<String, Open>>,
    input: Mutex<MIDIPortRef>,
}

impl Shared {
    /// After a notification: which sources came and went. An open one that went away is closed.
    fn rescan(&self) {
        let now: Vec<String> = sources().into_iter().map(|(n, _)| n).collect();
        let Ok(mut known) = self.known.lock() else {
            return;
        };
        for gone in known.iter().filter(|n| !now.contains(n)) {
            if let Ok(mut open) = self.open.lock() {
                open.remove(gone);
            }
            (self.sink)(PortEvent::Disconnected { name: gone.clone() });
        }
        for came in now.iter().filter(|n| !known.contains(n)) {
            (self.sink)(PortEvent::Connected { name: came.clone() });
        }
        *known = now;
    }

    fn receive(&self, list: *const c_void, source: MIDIEndpointRef) {
        // SAFETY: CoreMIDI passes a valid packet list to the read proc.
        let packets = unsafe { packets(list) };
        let mut events = Vec::new();
        if let Ok(mut open) = self.open.lock() {
            let Some((name, port)) = open.iter_mut().find(|(_, o)| o.source == source) else {
                return;
            };
            for (stamp, bytes) in packets {
                // SAFETY: no arguments.
                let ticks = if stamp == 0 {
                    unsafe { mach_absolute_time() }
                } else {
                    stamp
                };
                for message in split_channel_messages(&bytes, &mut port.running) {
                    events.push(PortEvent::Message {
                        name: name.clone(),
                        bytes: message,
                        host_ns: host_ns(ticks),
                    });
                }
            }
        }
        for event in events {
            (self.sink)(event);
        }
    }
}

extern "C" fn on_notification(notification: *const MIDINotification, refcon: *mut c_void) {
    // SAFETY: refcon is the Shared that outlives the client; the notification is valid for this call.
    let (shared, id) = unsafe { (&*(refcon as *const Shared), (*notification).message_id) };
    if matches!(id, SETUP_CHANGED | OBJECT_ADDED | OBJECT_REMOVED) {
        shared.rescan();
    }
}

extern "C" fn on_packets(list: *const c_void, refcon: *mut c_void, source: *mut c_void) {
    // SAFETY: refcon is the Shared that outlives the client.
    let shared = unsafe { &*(refcon as *const Shared) };
    shared.receive(list, source as usize as MIDIEndpointRef);
}

/// The CoreMIDI system of this process. Dropping it disposes the client and ends its thread.
pub struct CoreMidi {
    shared: Arc<Shared>,
    client: MIDIClientRef,
    output: MIDIPortRef,
    run_loop: usize,
    thread: Option<JoinHandle<()>>,
}

fn create_client(name: &CfString, refcon: *mut c_void) -> Result<MIDIClientRef, OSStatus> {
    let mut status = -1;
    for attempt in 0..10 {
        let mut client = 0;
        // SAFETY: refcon points to a Shared kept alive until the client is disposed.
        status = unsafe { MIDIClientCreate(name.0, Some(on_notification), refcon, &mut client) };
        if status == 0 {
            return Ok(client);
        }
        if !SERVER_STARTING.contains(&status) || attempt == 9 {
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(status)
}

impl CoreMidi {
    /// Opens a client called `name`. It reports every source present as `Connected` right away. Retries
    /// for a few seconds while the MIDI server is starting.
    pub fn new(name: &str, sink: Sink) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            sink,
            known: Mutex::new(Vec::new()),
            open: Mutex::new(BTreeMap::new()),
            input: Mutex::new(0),
        });
        type Ready = io::Result<(MIDIClientRef, MIDIPortRef, MIDIPortRef, usize)>;
        let (tx, rx) = mpsc::channel::<Ready>();
        let refcon = Arc::as_ptr(&shared) as usize;
        let name = name.to_owned();
        let thread = std::thread::Builder::new()
            .name("nu9ve-midi".into())
            .spawn(move || {
                let ready = (|| {
                    let client = create_client(&CfString::new(&name), refcon as *mut c_void)
                        .map_err(|s| io::Error::other(format!("MIDIClientCreate: {s}")))?;
                    let (mut input, mut output) = (0, 0);
                    let cin = CfString::new(&format!("{name} in"));
                    let cout = CfString::new(&format!("{name} out"));
                    // SAFETY: a live client; the read proc's refcon is the same Shared.
                    let ports = unsafe {
                        (
                            MIDIInputPortCreate(
                                client,
                                cin.0,
                                on_packets,
                                refcon as *mut c_void,
                                &mut input,
                            ),
                            MIDIOutputPortCreate(client, cout.0, &mut output),
                        )
                    };
                    if ports != (0, 0) {
                        // SAFETY: our client.
                        unsafe { MIDIClientDispose(client) };
                        return Err(io::Error::other(format!("MIDI ports: {ports:?}")));
                    }
                    // SAFETY: the run loop of this thread, valid while the thread lives.
                    let run_loop = unsafe { CFRunLoopGetCurrent() } as usize;
                    Ok((client, input, output, run_loop))
                })();
                let ok = ready.is_ok();
                let _ = tx.send(ready);
                if ok {
                    // CoreMIDI delivers its notifications on this run loop. It returns when stopped.
                    // SAFETY: runs this thread's own run loop.
                    unsafe { CFRunLoopRun() };
                }
            })?;
        let (client, input, output, run_loop) = rx
            .recv()
            .map_err(|_| io::Error::other("the MIDI thread ended"))??;
        if let Ok(mut port) = shared.input.lock() {
            *port = input;
        }
        shared.rescan();
        Ok(Self {
            shared,
            client,
            output,
            run_loop,
            thread: Some(thread),
        })
    }

    fn input(&self) -> MIDIPortRef {
        self.shared.input.lock().map(|p| *p).unwrap_or(0)
    }
}

impl MidiSystem for CoreMidi {
    fn sources(&self) -> Vec<String> {
        sources().into_iter().map(|(n, _)| n).collect()
    }

    fn open(&mut self, name: &str) -> io::Result<()> {
        let Some((_, source)) = sources().into_iter().find(|(n, _)| n == name) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no MIDI source {name}"),
            ));
        };
        let mut open = self
            .shared
            .open
            .lock()
            .map_err(|_| io::Error::other("poisoned"))?;
        if open.get(name).is_some_and(|o| o.source == source) {
            return Ok(());
        }
        if let Some(old) = open.remove(name) {
            // SAFETY: our input port and a source it was connected to.
            unsafe { MIDIPortDisconnectSource(self.input(), old.source) };
        }
        // SAFETY: our input port; the refcon is the endpoint, read back in the read proc.
        let status =
            unsafe { MIDIPortConnectSource(self.input(), source, source as usize as *mut c_void) };
        if status != 0 {
            return Err(io::Error::other(format!("MIDIPortConnectSource: {status}")));
        }
        open.insert(
            name.to_owned(),
            Open {
                source,
                destination: destination(name),
                running: None,
            },
        );
        Ok(())
    }

    fn close(&mut self, name: &str) {
        let removed = self
            .shared
            .open
            .lock()
            .ok()
            .and_then(|mut o| o.remove(name));
        if let Some(old) = removed {
            // SAFETY: our input port and a source it was connected to.
            unsafe { MIDIPortDisconnectSource(self.input(), old.source) };
        }
    }

    fn send(&mut self, name: &str, messages: &[Vec<u8>]) -> io::Result<()> {
        let destination = {
            let mut open = self
                .shared
                .open
                .lock()
                .map_err(|_| io::Error::other("poisoned"))?;
            let Some(port) = open.get_mut(name) else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{name} is not open"),
                ));
            };
            // A destination may appear after its source.
            if port.destination.is_none() {
                port.destination = destination(name);
            }
            port.destination
        };
        match destination {
            Some(d) => send_to(self.output, d, messages),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{name} has no destination for its lights"),
            )),
        }
    }
}

impl Drop for CoreMidi {
    fn drop(&mut self) {
        // SAFETY: our client; after this no callback runs, so the Shared can go.
        unsafe { MIDIClientDispose(self.client) };
        // SAFETY: the run loop of our thread, which is still alive until joined.
        unsafe { CFRunLoopStop(self.run_loop as CFRunLoopRef) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A virtual controller for tests: a source and a destination with the same name, in a client of its own.
/// What is sent to its destination is kept in order.
pub struct VirtualPort {
    client: MIDIClientRef,
    source: MIDIEndpointRef,
    destination: MIDIEndpointRef,
    received: Box<Mutex<Vec<Vec<u8>>>>,
}

extern "C" fn on_virtual(list: *const c_void, refcon: *mut c_void, _: *mut c_void) {
    // SAFETY: refcon is the boxed Mutex owned by the VirtualPort, alive until its client is disposed.
    let received = unsafe { &*(refcon as *const Mutex<Vec<Vec<u8>>>) };
    // SAFETY: CoreMIDI passes a valid packet list.
    let packets = unsafe { packets(list) };
    if let Ok(mut r) = received.lock() {
        r.extend(packets.into_iter().map(|(_, b)| b));
    }
}

extern "C" fn ignore_notification(_: *const MIDINotification, _: *mut c_void) {}

impl VirtualPort {
    pub fn new(name: &str) -> io::Result<Self> {
        let cf = CfString::new(name);
        let mut status = -1;
        let mut client = 0;
        for attempt in 0..10 {
            // SAFETY: a name and an out pointer; no refcon is used by the notify proc.
            status = unsafe {
                MIDIClientCreate(
                    cf.0,
                    Some(ignore_notification),
                    std::ptr::null_mut(),
                    &mut client,
                )
            };
            if status == 0 || !SERVER_STARTING.contains(&status) || attempt == 9 {
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        if status != 0 {
            return Err(io::Error::other(format!("MIDIClientCreate: {status}")));
        }
        let received = Box::new(Mutex::new(Vec::new()));
        let (mut source, mut destination) = (0, 0);
        // SAFETY: a live client; the refcon is the boxed Mutex kept by the returned value.
        let created = unsafe {
            (
                MIDISourceCreate(client, cf.0, &mut source),
                MIDIDestinationCreate(
                    client,
                    cf.0,
                    on_virtual,
                    &*received as *const Mutex<Vec<Vec<u8>>> as *mut c_void,
                    &mut destination,
                ),
            )
        };
        if created != (0, 0) {
            // SAFETY: our client.
            unsafe { MIDIClientDispose(client) };
            return Err(io::Error::other(format!("virtual endpoints: {created:?}")));
        }
        Ok(Self {
            client,
            source,
            destination,
            received,
        })
    }

    /// Plays bytes as if the controller sent them, all in one packet.
    pub fn play(&self, bytes: &[u8]) {
        let mut buffer = vec![0u64; 128];
        let list = buffer.as_mut_ptr() as *mut c_void;
        // SAFETY: a 1 KiB 8-aligned buffer and a short message.
        unsafe {
            let packet = MIDIPacketListInit(list);
            MIDIPacketListAdd(list, 1024, packet, 0, bytes.len().min(900), bytes.as_ptr());
            MIDIReceived(self.source, list);
        }
    }

    /// What the owner sent to this controller's lights so far.
    pub fn received(&self) -> Vec<Vec<u8>> {
        self.received.lock().map(|r| r.clone()).unwrap_or_default()
    }

    /// The controller goes away: its source and destination are removed.
    pub fn unplug(&mut self) {
        // SAFETY: our own endpoints, disposed once.
        unsafe {
            if self.source != 0 {
                MIDIEndpointDispose(self.source);
            }
            if self.destination != 0 {
                MIDIEndpointDispose(self.destination);
            }
        }
        self.source = 0;
        self.destination = 0;
    }
}

impl Drop for VirtualPort {
    fn drop(&mut self) {
        self.unplug();
        // SAFETY: our client; no callback runs after this, so the box can go.
        unsafe { MIDIClientDispose(self.client) };
    }
}
