use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDCheckAccess(request_type: u32) -> u32;
}

const KIO_HID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
const KIO_HID_ACCESS_TYPE_GRANTED: u32 = 0;

pub fn check_accessibility() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

pub fn prompt_accessibility() -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let value = core_foundation::boolean::CFBoolean::true_value();
        let dict = CFDictionary::from_CFType_pairs(&[(key, value)]);
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const _)
    }
}

pub fn check_input_monitoring() -> bool {
    unsafe { IOHIDCheckAccess(KIO_HID_REQUEST_TYPE_LISTEN_EVENT) == KIO_HID_ACCESS_TYPE_GRANTED }
}
