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

pub struct RetainedAssumedSendAndSync<T: ?Sized>(pub Retained<T>);

unsafe impl<T: ?Sized> Send for RetainedAssumedSendAndSync<T> {}
unsafe impl<T: ?Sized> Sync for RetainedAssumedSendAndSync<T> {}

impl<T: ?Sized> RetainedAssumedSendAndSync<T> {
    pub fn as_ref(&self) -> &T {
        &self.0
    }
}

static USER_NOTIFICATION_DELEGATE_IS_INSTALLED: AtomicBool = AtomicBool::new(false);
static USER_NOTIFICATION_YABAI_ICON: OnceLock<RetainedAssumedSendAndSync<NSImage>> =
    OnceLock::new();

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

fn install_user_notification_delegate_and_load_yabai_icon() -> bool {
    let delegate = NotifyDelegate::alloc().set_ivars(());
    let delegate: Retained<NotifyDelegate> = unsafe { msg_send![super(delegate), init] };
    unsafe {
        NSUserNotificationCenter::defaultUserNotificationCenter()
            .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }
    std::mem::forget(delegate);

    USER_NOTIFICATION_YABAI_ICON.get_or_init(|| {
        let executable_path = NSBundle::mainBundle().executablePath().unwrap();
        RetainedAssumedSendAndSync(
            NSWorkspace::sharedWorkspace()
                .iconForFile(&executable_path.stringByResolvingSymlinksInPath()),
        )
    });
    USER_NOTIFICATION_DELEGATE_IS_INSTALLED.store(true, Ordering::Relaxed);

    true
}

pub fn deliver_user_notification(subtitle: &str, informative_text: &str) {
    autoreleasepool(|_pool| {
        if !USER_NOTIFICATION_DELEGATE_IS_INSTALLED.load(Ordering::Relaxed) {
            install_user_notification_delegate_and_load_yabai_icon();
        }

        let notification = NSUserNotification::init(NSUserNotification::alloc());
        notification.setTitle(Some(&NSString::from_str("yabai")));
        notification.setSubtitle(Some(&NSString::from_str(subtitle)));
        notification.setInformativeText(Some(&NSString::from_str(informative_text)));

        let notify_image: &AnyObject = USER_NOTIFICATION_YABAI_ICON
            .get()
            .unwrap()
            .as_ref()
            .as_ref();
        let identity_image_has_border = NSNumber::new_bool(false);
        let identity_image_has_border: &NSNumber = &identity_image_has_border;
        let identity_image_has_border: &AnyObject = identity_image_has_border.as_ref();
        unsafe {
            notification.setValue_forKey(Some(notify_image), &NSString::from_str("_identityImage"));
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
        $crate::support::notify::deliver_user_notification($subtitle, &format!($($argument)*))
    };
}
