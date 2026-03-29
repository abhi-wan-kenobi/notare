use std::time::Instant;

use tokio::sync::mpsc;

#[derive(Debug)]
pub enum ShortcutEvent {
    RecordStart,
    RecordStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutErrorKind {
    Unsupported,
    InputMonitoringDenied,
    SecureKeyboardEntry,
    TapDisabled,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutError {
    kind: ShortcutErrorKind,
    message: &'static str,
}

impl ShortcutError {
    pub fn internal(message: &'static str) -> Self {
        internal_error(message)
    }

    pub fn kind(self) -> ShortcutErrorKind {
        self.kind
    }

    pub fn message(self) -> &'static str {
        self.message
    }

    pub fn recovery(self) -> &'static str {
        match self.kind {
            ShortcutErrorKind::Unsupported => "This feature is only available on macOS.",
            ShortcutErrorKind::InputMonitoringDenied => {
                "Open System Settings → Privacy & Security → Input Monitoring, enable char, and retry."
            }
            ShortcutErrorKind::SecureKeyboardEntry => {
                "Disable Secure Keyboard Entry in the app holding it, then retry."
            }
            ShortcutErrorKind::TapDisabled => {
                "Inspect the shortcut stderr log, then retry `char shortcut install`."
            }
            ShortcutErrorKind::Internal => "Inspect the shortcut stderr log and retry.",
        }
    }
}

pub struct ShortcutListener {
    pub events: mpsc::UnboundedReceiver<ShortcutEvent>,
    pub failures: mpsc::UnboundedReceiver<ShortcutError>,
}

pub fn current_blocker() -> Option<ShortcutError> {
    #[cfg(target_os = "macos")]
    {
        if secure_keyboard_entry_enabled() {
            return Some(secure_keyboard_entry_error());
        }
        probe_event_tap().err()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Some(unsupported_error())
    }
}

pub fn input_monitoring_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        probe_event_tap().is_ok()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn listen() -> Result<ShortcutListener, ShortcutError> {
    #[cfg(target_os = "macos")]
    {
        if let Some(blocker) = current_blocker() {
            return Err(blocker);
        }

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (failure_tx, failure_rx) = mpsc::unbounded_channel();
        let (startup_tx, startup_rx) = std::sync::mpsc::channel::<Result<(), ShortcutError>>();

        std::thread::spawn(move || {
            unsafe { run_event_tap(event_tx, failure_tx, startup_tx) };
        });

        match startup_rx.recv() {
            Ok(Ok(())) => Ok(ShortcutListener {
                events: event_rx,
                failures: failure_rx,
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(internal_error("Hotkey listener exited during startup.")),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(unsupported_error())
    }
}

fn secure_keyboard_entry_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::SecureKeyboardEntry,
        message: "Secure Keyboard Entry is enabled and blocks the global hotkey.",
    }
}

fn input_monitoring_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::InputMonitoringDenied,
        message: "Input Monitoring permission is required for the global hotkey.",
    }
}

fn tap_disabled_error(message: &'static str) -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::TapDisabled,
        message,
    }
}

fn internal_error(message: &'static str) -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::Internal,
        message,
    }
}

#[cfg(not(target_os = "macos"))]
fn unsupported_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::Unsupported,
        message: "Global shortcuts are only supported on macOS.",
    }
}

const DOUBLE_TAP_WINDOW_MS: u128 = 400;
const RIGHT_ALT_MASK: u64 = 0x00000040;

#[cfg(target_os = "macos")]
const CGEVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = 0xFFFFFFFE;
#[cfg(target_os = "macos")]
const CGEVENT_TAP_DISABLED_BY_USER: CGEventType = 0xFFFFFFFF;

#[cfg(target_os = "macos")]
enum TapState {
    Idle,
    FirstTap(Instant),
    Recording,
}

#[cfg(target_os = "macos")]
fn probe_event_tap() -> Result<(), ShortcutError> {
    unsafe extern "C" fn noop_callback(
        _proxy: CGEventTapProxy,
        _event_type: CGEventType,
        event: CGEventRef,
        _user_info: *mut std::ffi::c_void,
    ) -> CGEventRef {
        event
    }

    let event_mask = (1 << CGEVENT_FLAGS_CHANGED) as CGEventMask;
    let tap = unsafe {
        CGEventTapCreate(
            KCGSESSION_EVENT_TAP,
            KCGHEAD_INSERT_EVENT_TAP,
            KCGEVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            noop_callback,
            std::ptr::null_mut(),
        )
    };

    if tap.is_null() {
        return Err(input_monitoring_error());
    }
    let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    if run_loop_source.is_null() {
        unsafe { CFRelease(tap) };
        return Err(tap_disabled_error(
            "Failed to create a run loop source for the event tap.",
        ));
    }

    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        let result = if CGEventTapIsEnabled(tap) {
            Ok(())
        } else {
            Err(tap_disabled_error(
                "macOS created the event tap but left it disabled.",
            ))
        };
        CFRunLoopRemoveSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CFRelease(run_loop_source);
        CFRelease(tap);
        result
    }
}

#[cfg(target_os = "macos")]
unsafe fn run_event_tap(
    event_tx: mpsc::UnboundedSender<ShortcutEvent>,
    failure_tx: mpsc::UnboundedSender<ShortcutError>,
    startup_tx: std::sync::mpsc::Sender<Result<(), ShortcutError>>,
) {
    let mut state = TapState::Idle;
    let mut was_pressed = false;
    let mut tap_disabled = false;

    let state_ptr = &mut state as *mut TapState;
    let was_pressed_ptr = &mut was_pressed as *mut bool;
    let event_tx_ptr = &event_tx as *const mpsc::UnboundedSender<ShortcutEvent>;
    let failure_tx_ptr = &failure_tx as *const mpsc::UnboundedSender<ShortcutError>;
    let tap_disabled_ptr = &mut tap_disabled as *mut bool;

    struct CallbackData {
        state: *mut TapState,
        was_pressed: *mut bool,
        event_tx: *const mpsc::UnboundedSender<ShortcutEvent>,
        failure_tx: *const mpsc::UnboundedSender<ShortcutError>,
        tap_disabled: *mut bool,
    }

    let mut data = CallbackData {
        state: state_ptr,
        was_pressed: was_pressed_ptr,
        event_tx: event_tx_ptr,
        failure_tx: failure_tx_ptr,
        tap_disabled: tap_disabled_ptr,
    };

    unsafe extern "C" fn callback(
        _proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut std::ffi::c_void,
    ) -> CGEventRef {
        let data = unsafe { &mut *(user_info as *mut CallbackData) };

        if event_type == CGEVENT_TAP_DISABLED_BY_TIMEOUT
            || event_type == CGEVENT_TAP_DISABLED_BY_USER
        {
            let already_disabled = unsafe { *data.tap_disabled };
            if !already_disabled {
                unsafe { *data.tap_disabled = true };
                let failure_tx = unsafe { &*data.failure_tx };
                let _ = failure_tx.send(tap_disabled_error(
                    "macOS disabled the event tap while the daemon was running.",
                ));
            }
            unsafe { CFRunLoopStop(CFRunLoopGetCurrent()) };
            return event;
        }

        let flags = unsafe { CGEventGetFlags(event) };
        let is_pressed = (flags & RIGHT_ALT_MASK) != 0;
        let was = unsafe { *data.was_pressed };

        if is_pressed && !was {
            let state = unsafe { &mut *data.state };
            let event_tx = unsafe { &*data.event_tx };

            match state {
                TapState::Idle => *state = TapState::FirstTap(Instant::now()),
                TapState::FirstTap(first) => {
                    if first.elapsed().as_millis() <= DOUBLE_TAP_WINDOW_MS {
                        let _ = event_tx.send(ShortcutEvent::RecordStart);
                        *state = TapState::Recording;
                    } else {
                        *state = TapState::FirstTap(Instant::now());
                    }
                }
                TapState::Recording => {
                    let _ = event_tx.send(ShortcutEvent::RecordStop);
                    *state = TapState::Idle;
                }
            }
        }

        unsafe { *data.was_pressed = is_pressed };
        event
    }

    let event_mask = (1 << CGEVENT_FLAGS_CHANGED) as CGEventMask;
    let tap = unsafe {
        CGEventTapCreate(
            KCGSESSION_EVENT_TAP,
            KCGHEAD_INSERT_EVENT_TAP,
            KCGEVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            callback,
            &mut data as *mut CallbackData as *mut std::ffi::c_void,
        )
    };

    if tap.is_null() {
        let _ = startup_tx.send(Err(input_monitoring_error()));
        return;
    }

    let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    if run_loop_source.is_null() {
        let _ = startup_tx.send(Err(tap_disabled_error(
            "Failed to create a run loop source for the event tap.",
        )));
        unsafe { CFRelease(tap) };
        return;
    }

    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        if !CGEventTapIsEnabled(tap) {
            let _ = startup_tx.send(Err(tap_disabled_error(
                "macOS created the event tap but left it disabled.",
            )));
            CFRunLoopRemoveSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
            CFMachPortInvalidate(tap);
            CFRelease(run_loop_source);
            CFRelease(tap);
            return;
        }

        let _ = startup_tx.send(Ok(()));
        CFRunLoopRun();
        CFRunLoopRemoveSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CFMachPortInvalidate(tap);
        CFRelease(run_loop_source);
        CFRelease(tap);
    }

    if !tap_disabled {
        let _ = failure_tx.send(internal_error("Hotkey listener exited unexpectedly."));
    }
}

#[cfg(target_os = "macos")]
fn secure_keyboard_entry_enabled() -> bool {
    unsafe { CGSIsSecureEventInputSet() }
}

#[cfg(target_os = "macos")]
type CGEventTapProxy = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CGEventRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CGEventMask = u64;
#[cfg(target_os = "macos")]
type CGEventType = u32;
#[cfg(target_os = "macos")]
type CFMachPortRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFRunLoopSourceRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFRunLoopRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFAllocatorRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFIndex = isize;

#[cfg(target_os = "macos")]
const KCGSESSION_EVENT_TAP: u32 = 1;
#[cfg(target_os = "macos")]
const KCGHEAD_INSERT_EVENT_TAP: u32 = 0;
#[cfg(target_os = "macos")]
const KCGEVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
#[cfg(target_os = "macos")]
const CGEVENT_FLAGS_CHANGED: u32 = 12;

#[cfg(target_os = "macos")]
type CGEventTapCallBack = unsafe extern "C" fn(
    CGEventTapProxy,
    CGEventType,
    CGEventRef,
    *mut std::ffi::c_void,
) -> CGEventRef;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: CGEventMask,
        callback: CGEventTapCallBack,
        user_info: *mut std::ffi::c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFMachPortInvalidate(port: CFMachPortRef);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);

    static kCFRunLoopCommonModes: CFStringRef;

    fn CFRelease(cf: *mut std::ffi::c_void);
    fn CGSIsSecureEventInputSet() -> bool;
}
