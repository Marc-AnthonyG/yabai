#![allow(deprecated)]
#![allow(non_snake_case)]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, define_class, msg_send};

use crate::ffi::appkit::{NSImage, NSWorkspace};
use crate::ffi::foundation::{
    NSBundle, NSNumber, NSObject, NSObjectNSKeyValueCoding, NSString, NSUserNotification,
    NSUserNotificationCenter, NSUserNotificationCenterDelegate,
};

pub struct SendRetained<T: ?Sized>(pub Retained<T>);

unsafe impl<T: ?Sized> Send for SendRetained<T> {}
unsafe impl<T: ?Sized> Sync for SendRetained<T> {}

impl<T: ?Sized> SendRetained<T> {
    pub fn as_ref(&self) -> &T {
        &self.0
    }
}

static NOTIFY_INIT: AtomicBool = AtomicBool::new(false);
static NOTIFY_IMAGE: OnceLock<SendRetained<NSImage>> = OnceLock::new();

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "NotifyDelegate"]
    struct NotifyDelegate;

    unsafe impl NSObjectProtocol for NotifyDelegate {}

    unsafe impl NSUserNotificationCenterDelegate for NotifyDelegate {
        #[unsafe(method(userNotificationCenter:shouldPresentNotification:))]
        fn userNotificationCenter_shouldPresentNotification(
            &self,
            _center: &NSUserNotificationCenter,
            _notification: &NSUserNotification,
        ) -> bool {
            true
        }
    }
);

fn notify_init() -> bool {
    let delegate = NotifyDelegate::alloc().set_ivars(());
    let delegate: Retained<NotifyDelegate> = unsafe { msg_send![super(delegate), init] };
    unsafe {
        NSUserNotificationCenter::defaultUserNotificationCenter()
            .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }
    std::mem::forget(delegate);

    NOTIFY_IMAGE.get_or_init(|| {
        let executable_path = NSBundle::mainBundle().executablePath().unwrap();
        SendRetained(
            NSWorkspace::sharedWorkspace()
                .iconForFile(&executable_path.stringByResolvingSymlinksInPath()),
        )
    });
    NOTIFY_INIT.store(true, Ordering::Relaxed);

    true
}

pub fn notify(subtitle: &str, informative_text: &str) {
    autoreleasepool(|_pool| {
        if !NOTIFY_INIT.load(Ordering::Relaxed) {
            notify_init();
        }

        let notification = NSUserNotification::init(NSUserNotification::alloc());
        notification.setTitle(Some(&NSString::from_str("yabai")));
        notification.setSubtitle(Some(&NSString::from_str(subtitle)));
        notification.setInformativeText(Some(&NSString::from_str(informative_text)));

        let notify_image: &AnyObject = NOTIFY_IMAGE.get().unwrap().as_ref().as_ref();
        let identity_image_has_border = NSNumber::new_bool(false);
        let identity_image_has_border: &NSNumber = &identity_image_has_border;
        let identity_image_has_border: &AnyObject = identity_image_has_border.as_ref();
        unsafe {
            notification
                .setValue_forKey(Some(notify_image), &NSString::from_str("_identityImage"));
            notification.setValue_forKey(
                Some(identity_image_has_border),
                &NSString::from_str("_identityImageHasBorder"),
            );
        }

        NSUserNotificationCenter::defaultUserNotificationCenter()
            .deliverNotification(&notification);
    });
}

#[macro_export]
macro_rules! notify {
    ($subtitle:expr, $($argument:tt)*) => {
        $crate::support::notify::notify($subtitle, &format!($($argument)*))
    };
}
