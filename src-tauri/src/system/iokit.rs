//! Thin wrappers over the IOKit registry. Unsupported keys come back as `None`.

use std::ffi::CString;
use std::ptr;

use core_foundation::base::{kCFAllocatorDefault, CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFMutableDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::types::{io_iterator_t, io_object_t};
use io_kit_sys::{
    kIOMasterPortDefault, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperties,
    IOServiceGetMatchingServices, IOServiceMatching,
};

pub struct Props(CFDictionary<CFString, CFType>);

impl Props {
    pub fn i64(&self, key: &str) -> Option<i64> {
        self.0
            .find(CFString::new(key))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.0
            .find(CFString::new(key))
            .and_then(|v| v.downcast::<CFBoolean>())
            .map(bool::from)
    }

    pub fn dict(&self, key: &str) -> Option<Props> {
        self.0.find(CFString::new(key)).and_then(|v| {
            // SAFETY: a CFDictionaryGetTypeID match guarantees the cast; wrap_under_get retains.
            unsafe {
                use core_foundation::base::{CFTypeRef, TCFTypeRef};
                use core_foundation::dictionary::{CFDictionaryGetTypeID, CFDictionaryRef};
                if core_foundation::base::CFGetTypeID(v.as_CFTypeRef()) != CFDictionaryGetTypeID() {
                    return None;
                }
                let raw: CFTypeRef = v.as_CFTypeRef();
                let dict: CFDictionary<CFString, CFType> =
                    CFDictionary::wrap_under_get_rule(raw.as_void_ptr() as CFDictionaryRef);
                Some(Props(dict))
            }
        })
    }
}

/// Reads the full property table of every service registered under `class`.
pub fn services(class: &str) -> Vec<Props> {
    let Ok(class) = CString::new(class) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    // SAFETY: standard IOKit iteration. The matching dictionary is consumed by
    // IOServiceGetMatchingServices; each entry and the iterator are released here.
    unsafe {
        let matching = IOServiceMatching(class.as_ptr());
        if matching.is_null() {
            return found;
        }
        let mut iterator: io_iterator_t = 0;
        if IOServiceGetMatchingServices(kIOMasterPortDefault, matching, &mut iterator) != 0 {
            return found;
        }
        loop {
            let entry: io_object_t = IOIteratorNext(iterator);
            if entry == 0 {
                break;
            }
            let mut props: CFMutableDictionaryRef = ptr::null_mut();
            let rc = IORegistryEntryCreateCFProperties(entry, &mut props, kCFAllocatorDefault, 0);
            if rc == 0 && !props.is_null() {
                found.push(Props(CFDictionary::wrap_under_create_rule(props as _)));
            }
            IOObjectRelease(entry);
        }
        IOObjectRelease(iterator);
    }
    found
}
