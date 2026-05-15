//! macOS-specific support for file associations (Issue #67).
//!
//! When a user double-clicks a JSON file in Finder or uses "Open With → Thoth",
//! macOS sends an `application:openURLs:` message to the `NSApplication`
//! delegate. Winit's `WinitApplicationDelegate` doesn't implement this method,
//! so `NSApplication` falls back to showing a "cannot open" error dialog.
//!
//! ## Strategy
//!
//! We dynamically add `application:openURLs:` to `NSObject` at startup — before
//! `eframe::run_native` creates the event loop. Because `WinitApplicationDelegate`
//! inherits from `NSObject` and doesn't override this selector, the Objective-C
//! runtime walks the class hierarchy and finds our implementation. When the
//! handler fires, it enqueues file paths into the cross-platform
//! [`file_open_channel`](super::file_open_channel) queue, which
//! `ThothApp::poll_os_open_requests()` drains each frame.
//!
//! An `NSAppleEventManager` handler is also registered as a belt-and-suspenders
//! fallback for the `kAEOpenDocuments` (`odoc`) Apple Event.

use crate::platform::file_open_channel::enqueue_open_request;
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{msg_send, sel};
use std::ffi::CString;
use std::path::PathBuf;

unsafe extern "C" {
    fn class_addMethod(
        cls: *const AnyClass,
        name: Sel,
        imp: unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
        types: *const std::ffi::c_char,
    ) -> Bool;
}

/// Objective-C method implementation for `application:openURLs:`.
///
/// Called by the runtime when macOS dispatches file-open events to the
/// `NSApplication` delegate. Extracts POSIX paths from each `NSURL` in the
/// array and enqueues them via [`enqueue_open_request`].
unsafe extern "C" fn handle_open_urls(
    _this: *mut AnyObject,
    _cmd: Sel,
    _application: *mut AnyObject,
    urls: *mut AnyObject,
) {
    let count: usize = unsafe { msg_send![urls, count] };
    eprintln!("[thoth] macOS: application:openURLs: received {count} URL(s)");

    for i in 0..count {
        let url: *mut AnyObject = unsafe { msg_send![urls, objectAtIndex: i] };
        if url.is_null() {
            continue;
        }
        let path_nsstring: *mut AnyObject = unsafe { msg_send![url, path] };
        if path_nsstring.is_null() {
            continue;
        }
        let utf8: *const u8 = unsafe { msg_send![path_nsstring, UTF8String] };
        if utf8.is_null() {
            continue;
        }
        let c_str = unsafe { std::ffi::CStr::from_ptr(utf8 as *const _) };
        if let Ok(s) = c_str.to_str() {
            let path = PathBuf::from(s);
            eprintln!("[thoth] macOS: opening {}", path.display());
            enqueue_open_request(path);
        }
    }
}

/// Install all macOS file association handlers.
///
/// Must be called **before** `eframe::run_native` so the handlers are in place
/// when macOS delivers the initial `odoc` Apple Event during cold launch.
pub fn install_all_handlers() {
    install_delegate_method();
    install_apple_event_handler();
}

/// Add `application:openURLs:` to `NSObject`.
///
/// `WinitApplicationDelegate` inherits from `NSObject` and doesn't implement
/// this selector, so the Objective-C runtime finds our implementation via the
/// class hierarchy when `NSApplication` dispatches the message.
fn install_delegate_method() {
    let Some(cls) = objc2::runtime::AnyClass::get("NSObject") else {
        eprintln!("[thoth] macOS: NSObject class not found (unexpected)");
        return;
    };

    let open_sel = sel!(application:openURLs:);
    // ObjC type encoding: void (id self, SEL _cmd, NSApplication*, NSArray*)
    let types = CString::new("v@:@@").unwrap();

    let added =
        unsafe { class_addMethod(cls as *const _, open_sel, handle_open_urls, types.as_ptr()) };

    if added.as_bool() {
        eprintln!("[thoth] macOS: application:openURLs: installed on NSObject");
    } else {
        eprintln!("[thoth] macOS: application:openURLs: already exists on NSObject");
    }
}

/// Register an `NSAppleEventManager` handler for `kAEOpenDocuments` as a
/// fallback in case the delegate-based approach doesn't fire.
fn install_apple_event_handler() {
    unsafe {
        let Some(nseam_class) = objc2::runtime::AnyClass::get("NSAppleEventManager") else {
            return;
        };
        let mgr: *mut AnyObject = msg_send![nseam_class, sharedAppleEventManager];

        let event_class: u32 = 0x6165_7674; // 'aevt' (kCoreEventClass)
        let event_id: u32 = 0x6f64_6f63; // 'odoc' (kAEOpenDocuments)

        // Create a one-off handler class if it doesn't already exist.
        if objc2::runtime::AnyClass::get("ThothOpenDocHandler").is_none() {
            let superclass = objc2::runtime::AnyClass::get("NSObject").unwrap();
            let mut builder =
                objc2::runtime::ClassBuilder::new("ThothOpenDocHandler", superclass).unwrap();

            unsafe extern "C" fn handle_apple_event(
                _this: *mut AnyObject,
                _cmd: Sel,
                event: *mut AnyObject,
                _reply: *mut AnyObject,
            ) {
                eprintln!("[thoth] macOS: NSAppleEventManager odoc handler fired");
                // keyDirectObject = '----' = 0x2d2d2d2d
                let desc: *mut AnyObject =
                    msg_send![event, paramDescriptorForKeyword: 0x2d2d_2d2du32];
                if desc.is_null() {
                    return;
                }
                let count: i32 = msg_send![desc, numberOfItems];
                for i in 1..=count {
                    let item: *mut AnyObject = msg_send![desc, descriptorAtIndex: i];
                    if item.is_null() {
                        continue;
                    }
                    let url_string: *mut AnyObject = msg_send![item, stringValue];
                    if url_string.is_null() {
                        continue;
                    }
                    let utf8: *const u8 = msg_send![url_string, UTF8String];
                    if utf8.is_null() {
                        continue;
                    }
                    let c_str = unsafe { std::ffi::CStr::from_ptr(utf8 as *const _) };
                    if let Ok(s) = c_str.to_str() {
                        let path = if s.starts_with("file://") {
                            // Convert file:// URL to POSIX path via NSURL.
                            let ns_string_class =
                                objc2::runtime::AnyClass::get("NSString").unwrap();
                            let ns_url_class = objc2::runtime::AnyClass::get("NSURL").unwrap();
                            let ns_str: *mut AnyObject =
                                msg_send![ns_string_class, stringWithUTF8String: utf8];
                            let url: *mut AnyObject =
                                msg_send![ns_url_class, URLWithString: ns_str];
                            if url.is_null() {
                                continue;
                            }
                            let path_obj: *mut AnyObject = msg_send![url, path];
                            if path_obj.is_null() {
                                continue;
                            }
                            let path_utf8: *const u8 = msg_send![path_obj, UTF8String];
                            if path_utf8.is_null() {
                                continue;
                            }
                            let path_cstr =
                                unsafe { std::ffi::CStr::from_ptr(path_utf8 as *const _) };
                            match path_cstr.to_str() {
                                Ok(p) => PathBuf::from(p),
                                Err(_) => continue,
                            }
                        } else {
                            PathBuf::from(s)
                        };
                        eprintln!("[thoth] macOS: opening {}", path.display());
                        enqueue_open_request(path);
                    }
                }
            }

            builder.add_method(
                sel!(handleOpenEvent:withReplyEvent:),
                handle_apple_event as unsafe extern "C" fn(_, _, _, _),
            );
            let _ = builder.register();
        }

        let handler_class = objc2::runtime::AnyClass::get("ThothOpenDocHandler").unwrap();
        let handler: *mut AnyObject = msg_send![handler_class, new];

        // Prevent deallocation by leaking the handler into a static.
        static mut HANDLER: *mut AnyObject = std::ptr::null_mut();
        HANDLER = handler;

        let _: () = msg_send![mgr,
            setEventHandler: handler
            andSelector: sel!(handleOpenEvent:withReplyEvent:)
            forEventClass: event_class
            andEventID: event_id
        ];

        eprintln!("[thoth] macOS: NSAppleEventManager odoc handler registered");
    }
}
