use std::time::Instant;

use tokio::sync::mpsc;

#[derive(Debug)]
pub(crate) enum HotkeyEvent {
    RecordStart,
    RecordStop,
}

// Double-tap detection window
const DOUBLE_TAP_WINDOW_MS: u128 = 400;

// Right Option key mask (NX_DEVICERALTKEYMASK)
const RIGHT_ALT_MASK: u64 = 0x00000040;

enum TapState {
    Idle,
    FirstTap(Instant),
    Recording,
}

pub(crate) enum ProbeResult {
    Ok,
    Denied,
}

/// Try to create an event tap to check if Input Monitoring permission is granted.
pub(crate) fn probe_event_tap() -> ProbeResult {
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
            KCGHID_EVENT_TAP,
            KCGHEAD_INSERT_EVENT_TAP,
            KCGEVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            noop_callback,
            std::ptr::null_mut(),
        )
    };

    if tap.is_null() {
        ProbeResult::Denied
    } else {
        // Tap created successfully — permission is granted. Clean up.
        unsafe { CFRelease(tap) };
        ProbeResult::Ok
    }
}

pub(crate) fn listen() -> mpsc::UnboundedReceiver<HotkeyEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        unsafe { run_event_tap(tx) };
    });

    rx
}

unsafe fn run_event_tap(tx: mpsc::UnboundedSender<HotkeyEvent>) {
    let mut state = TapState::Idle;
    let mut was_pressed = false;

    let state_ptr = &mut state as *mut TapState;
    let was_pressed_ptr = &mut was_pressed as *mut bool;
    let tx_ptr = &tx as *const mpsc::UnboundedSender<HotkeyEvent>;

    // We need to use a C callback, so we pack our state into the user_info pointer
    struct CallbackData {
        state: *mut TapState,
        was_pressed: *mut bool,
        tx: *const mpsc::UnboundedSender<HotkeyEvent>,
    }

    let mut data = CallbackData {
        state: state_ptr,
        was_pressed: was_pressed_ptr,
        tx: tx_ptr,
    };

    unsafe extern "C" fn callback(
        _proxy: CGEventTapProxy,
        _event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut std::ffi::c_void,
    ) -> CGEventRef {
        let data = unsafe { &mut *(user_info as *mut CallbackData) };
        let flags = unsafe { CGEventGetFlags(event) };
        let is_pressed = (flags & RIGHT_ALT_MASK) != 0;
        let was = unsafe { *data.was_pressed };

        // Detect key-down edge (released → pressed)
        if is_pressed && !was {
            let state = unsafe { &mut *data.state };
            let tx = unsafe { &*data.tx };

            match state {
                TapState::Idle => {
                    *state = TapState::FirstTap(Instant::now());
                }
                TapState::FirstTap(first) => {
                    if first.elapsed().as_millis() <= DOUBLE_TAP_WINDOW_MS {
                        let _ = tx.send(HotkeyEvent::RecordStart);
                        *state = TapState::Recording;
                    } else {
                        *state = TapState::FirstTap(Instant::now());
                    }
                }
                TapState::Recording => {
                    let _ = tx.send(HotkeyEvent::RecordStop);
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
            KCGHID_EVENT_TAP,
            KCGHEAD_INSERT_EVENT_TAP,
            KCGEVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            callback,
            &mut data as *mut CallbackData as *mut std::ffi::c_void,
        )
    };

    if tap.is_null() {
        tracing::error!("Failed to create event tap. Is Input Monitoring permission granted?");
        return;
    }

    let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };

    if run_loop_source.is_null() {
        tracing::error!("Failed to create run loop source");
        return;
    }

    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        tracing::info!("Hotkey listener started (double-tap Right Option)");
        CFRunLoopRun();
    }
}

// CoreGraphics event tap FFI
type CGEventTapProxy = *mut std::ffi::c_void;
type CGEventRef = *mut std::ffi::c_void;
type CGEventMask = u64;
type CGEventType = u32;
type CFMachPortRef = *mut std::ffi::c_void;
type CFRunLoopSourceRef = *mut std::ffi::c_void;
type CFRunLoopRef = *mut std::ffi::c_void;
type CFAllocatorRef = *const std::ffi::c_void;
type CFStringRef = *const std::ffi::c_void;
type CFIndex = isize;

const KCGHID_EVENT_TAP: u32 = 0;
const KCGHEAD_INSERT_EVENT_TAP: u32 = 0;
const KCGEVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const CGEVENT_FLAGS_CHANGED: u32 = 12;

type CGEventTapCallBack = unsafe extern "C" fn(
    CGEventTapProxy,
    CGEventType,
    CGEventRef,
    *mut std::ffi::c_void,
) -> CGEventRef;

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
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();

    static kCFRunLoopCommonModes: CFStringRef;

    fn CFRelease(cf: *mut std::ffi::c_void);
}
