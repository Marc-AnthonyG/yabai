#![allow(non_snake_case)]

use core::ffi::c_void;
use core::mem::ManuallyDrop;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send, sel};

use crate::debug;
use crate::event::queue::{EVENT_SENDER, Event};
use crate::ffi::appkit::{
    APPLE_INTERFACE_MENU_BAR_HIDING_CHANGED_NOTIFICATION, COM_APPLE_DOCK_PREFCHANGED,
    NS_APPLICATION_DOCK_DID_RESTART_NOTIFICATION,
    NS_WORKSPACE_ACTIVE_DISPLAY_DID_CHANGE_NOTIFICATION, NSRunningApplication, NSWorkspace,
    NSWorkspaceActiveSpaceDidChangeNotification, NSWorkspaceApplicationKey,
    NSWorkspaceDidHideApplicationNotification, NSWorkspaceDidUnhideApplicationNotification,
    NSWorkspaceDidWakeNotification,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::foundation::{
    NSDictionary, NSDistributedNotificationCenter, NSKeyValueChangeNewKey,
    NSKeyValueObservingOptions, NSNotification, NSNotificationCenter, NSObject,
    NSObjectNSKeyValueObserverRegistration, NSProcessInfo, NSString,
};
use crate::process::model::Process;
use crate::support::handles::ProcessId;
use crate::support::macos_version::{
    _workspace_is_macos_version_bigsur, _workspace_is_macos_version_monterey,
    _workspace_is_macos_version_sequoia, _workspace_is_macos_version_sonoma,
    _workspace_is_macos_version_tahoe, _workspace_is_macos_version_ventura,
    supported_macos_version_list,
};

pub(crate) struct WorkspaceContextIvars {
    event_sender: Sender<Event>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "workspace_context"]
    #[ivars = WorkspaceContextIvars]
    pub(crate) struct WorkspaceContext;

    impl WorkspaceContext {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observeValueForKeyPath_ofObject_change_context(
            &self,
            key_path: &NSString,
            object: &AnyObject,
            change: &NSDictionary,
            context: *mut c_void,
        ) {
            if key_path.isEqualToString(&NSString::from_str("activationPolicy")) {
                let process = ManuallyDrop::new(unsafe { Arc::from_raw(context as *const Process) });
                if process.terminated.load(Ordering::Acquire) {
                    return;
                }

                let result = change.objectForKey(unsafe { NSKeyValueChangeNewKey });
                let result = result.as_deref();
                if result.map_or(0, |result| unsafe { msg_send![result, intValue] })
                    != process.policy.load(Ordering::Relaxed)
                {
                    //
                    // :WorstApiEverMade
                    //
                    // NOTE(asmvik): For some stupid reason it is possible to get notified by the system
                    // about a change, and NOT being able to remove ourselves from observation because
                    // it claims that we are not observing the key-path, but we clearly are, as we would
                    // otherwise not be here in the first place..
                    //

                    let application = unsafe { &*(object as *const AnyObject).cast::<NSRunningApplication>() };
                    if remove_observer_swallowing_exception(
                        application,
                        self,
                        &NSString::from_str("activationPolicy"),
                        &process,
                    ) {
                        release_kvo_refcon_on_main_queue(&process);
                    }

                    process.policy.store(
                        result.map_or(0, |result| unsafe { msg_send![result, intValue] }),
                        Ordering::Relaxed,
                    );
                    debug!(
                        "{}: activation policy changed for {} ({})\n",
                        "-[workspace_context observeValueForKeyPath:ofObject:change:context:]",
                        process.name,
                        process.process_id.0
                    );
                    let _ = self
                        .ivars()
                        .event_sender
                        .send(Event::ApplicationLaunched(Arc::clone(&process)));
                }
            }

            if key_path.isEqualToString(&NSString::from_str("finishedLaunching")) {
                let process = ManuallyDrop::new(unsafe { Arc::from_raw(context as *const Process) });
                if process.terminated.load(Ordering::Acquire) {
                    return;
                }

                let result = change.objectForKey(unsafe { NSKeyValueChangeNewKey });
                let result = result.as_deref();
                if result.map_or(0, |result| unsafe { msg_send![result, intValue] }) == 1 {
                    //
                    // :WorstApiEverMade
                    //
                    // NOTE(asmvik): For some stupid reason it is possible to get notified by the system
                    // about a change, and NOT being able to remove ourselves from observation because
                    // it claims that we are not observing the key-path, but we clearly are, as we would
                    // otherwise not be here in the first place..
                    //

                    let application = unsafe { &*(object as *const AnyObject).cast::<NSRunningApplication>() };
                    if remove_observer_swallowing_exception(
                        application,
                        self,
                        &NSString::from_str("finishedLaunching"),
                        &process,
                    ) {
                        release_kvo_refcon_on_main_queue(&process);
                    }

                    debug!(
                        "{}: {} ({}) finished launching\n",
                        "-[workspace_context observeValueForKeyPath:ofObject:change:context:]",
                        process.name,
                        process.process_id.0
                    );
                    let _ = self
                        .ivars()
                        .event_sender
                        .send(Event::ApplicationLaunched(Arc::clone(&process)));
                }
            }
        }

        #[unsafe(method(didWake:))]
        fn didWake(&self, notification: &NSNotification) {
            let _ = self.ivars().event_sender.send(Event::SystemWoke);
        }

        #[unsafe(method(didChangeMenuBarHiding:))]
        fn didChangeMenuBarHiding(&self, notification: &NSNotification) {
            let _ = self.ivars().event_sender.send(Event::MenuBarHiddenChanged);
        }

        #[unsafe(method(didRestartDock:))]
        fn didRestartDock(&self, notification: &NSNotification) {
            let _ = self.ivars().event_sender.send(Event::DockDidRestart);
        }

        #[unsafe(method(didChangeDockPref:))]
        fn didChangeDockPref(&self, notification: &NSNotification) {
            let _ = self.ivars().event_sender.send(Event::DockDidChangePref);
        }

        #[unsafe(method(activeDisplayDidChange:))]
        fn activeDisplayDidChange(&self, notification: &NSNotification) {
            let _ = self.ivars().event_sender.send(Event::DisplayChanged);
        }

        #[unsafe(method(activeSpaceDidChange:))]
        fn activeSpaceDidChange(&self, notification: &NSNotification) {
            let _ = self.ivars().event_sender.send(Event::SpaceChanged);
        }

        #[unsafe(method(didHideApplication:))]
        fn didHideApplication(&self, notification: &NSNotification) {
            let process_id: libc::pid_t = notification
                .userInfo()
                .and_then(|user_info| user_info.objectForKey(unsafe { NSWorkspaceApplicationKey }))
                .map_or(0, |application| unsafe { msg_send![&*application, processIdentifier] });
            let _ = self
                .ivars()
                .event_sender
                .send(Event::ApplicationHidden(ProcessId(process_id)));
        }

        #[unsafe(method(didUnhideApplication:))]
        fn didUnhideApplication(&self, notification: &NSNotification) {
            let process_id: libc::pid_t = notification
                .userInfo()
                .and_then(|user_info| user_info.objectForKey(unsafe { NSWorkspaceApplicationKey }))
                .map_or(0, |application| unsafe { msg_send![&*application, processIdentifier] });
            let _ = self
                .ivars()
                .event_sender
                .send(Event::ApplicationVisible(ProcessId(process_id)));
        }
    }
);

pub(crate) static WORKSPACE_CONTEXT: OnceLock<Retained<WorkspaceContext>> = OnceLock::new();

impl WorkspaceContext {
    pub(crate) fn init(this: Allocated<Self>) -> Retained<Self> {
        let this = this.set_ivars(WorkspaceContextIvars {
            event_sender: EVENT_SENDER.get().unwrap().clone(),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        unsafe {
            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserver_selector_name_object(
                    &this,
                    sel!(activeDisplayDidChange:),
                    Some(&NSString::from_str(
                        NS_WORKSPACE_ACTIVE_DISPLAY_DID_CHANGE_NOTIFICATION,
                    )),
                    None,
                );

            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserver_selector_name_object(
                    &this,
                    sel!(activeSpaceDidChange:),
                    Some(NSWorkspaceActiveSpaceDidChangeNotification),
                    None,
                );

            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserver_selector_name_object(
                    &this,
                    sel!(didHideApplication:),
                    Some(NSWorkspaceDidHideApplicationNotification),
                    None,
                );

            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserver_selector_name_object(
                    &this,
                    sel!(didUnhideApplication:),
                    Some(NSWorkspaceDidUnhideApplicationNotification),
                    None,
                );

            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserver_selector_name_object(
                    &this,
                    sel!(didWake:),
                    Some(NSWorkspaceDidWakeNotification),
                    None,
                );

            NSDistributedNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &this,
                sel!(didChangeMenuBarHiding:),
                Some(&NSString::from_str(
                    APPLE_INTERFACE_MENU_BAR_HIDING_CHANGED_NOTIFICATION,
                )),
                None,
            );

            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &this,
                sel!(didRestartDock:),
                Some(&NSString::from_str(NS_APPLICATION_DOCK_DID_RESTART_NOTIFICATION)),
                None,
            );

            NSDistributedNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &this,
                sel!(didChangeDockPref:),
                Some(&NSString::from_str(COM_APPLE_DOCK_PREFCHANGED)),
                None,
            );
        }

        this
    }
}

impl Drop for WorkspaceContext {
    fn drop(&mut self) {
        unsafe {
            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .removeObserver(self);
            NSNotificationCenter::defaultCenter().removeObserver(self);
            NSDistributedNotificationCenter::defaultCenter().removeObserver(self);
        }
    }
}

pub(crate) fn workspace_event_handler_begin() -> bool {
    let version = NSProcessInfo::processInfo().operatingSystemVersion();
    macro_rules! support_macos_version_assignment {
        ($name:ident, $flag_name:ident, $accessor_name:ident, $major_version:literal) => {
            $flag_name.store(version.majorVersion == $major_version, Ordering::Relaxed);
        };
    }
    supported_macos_version_list!(support_macos_version_assignment);

    let workspace_context = WorkspaceContext::alloc();
    if Allocated::as_ptr(&workspace_context).is_null() {
        return false;
    }

    let workspace_context = WorkspaceContext::init(workspace_context);
    let _ = WORKSPACE_CONTEXT.set(workspace_context);

    true
}

pub(crate) fn workspace_application_destroy_running_ns_application(
    workspace_context: &WorkspaceContext,
    process: &Arc<Process>,
) {
    let application = process.ns_application.load(Ordering::Relaxed);

    if let Some(application) =
        unsafe { Retained::from_raw(application.cast::<NSRunningApplication>()) }
    {
        let observation_info: *mut c_void = unsafe { msg_send![&*application, observationInfo] };
        if !observation_info.is_null() {

            //
            // :WorstApiEverMade
            //
            // NOTE(asmvik): Because the developers of this API did such an amazing job
            // there is no way for us to actually just friggin loop through the currently
            // registered observations and then call removeObservation on them..
            //
            // Instead we just try to force remove the observations that **could** be present
            // at this point in time, because it will complain if we try to actually release
            // the object when it has observers present.
            //
            // We can't actually correctly track whether it did actually get unobserved previously,
            // because even when our notification callback is triggered it will claim that we try
            // to remove a non-existing observation when it just called us back.
            //

            if remove_observer_swallowing_exception(
                &application,
                workspace_context,
                &NSString::from_str("activationPolicy"),
                process,
            ) {
                release_kvo_refcon_on_main_queue(process);
            }

            if remove_observer_swallowing_exception(
                &application,
                workspace_context,
                &NSString::from_str("finishedLaunching"),
                process,
            ) {
                release_kvo_refcon_on_main_queue(process);
            }
        }

        drop(application);
    }
}

pub(crate) fn workspace_application_observe_finished_launching(
    context: &WorkspaceContext,
    process: &Arc<Process>,
) {
    let application = process.ns_application.load(Ordering::Relaxed);
    if let Some(application) = unsafe { application.cast::<NSRunningApplication>().as_ref() } {
        let refcon = Arc::into_raw(Arc::clone(process)) as *mut c_void;
        unsafe {
            application.addObserver_forKeyPath_options_context(
                context,
                &NSString::from_str("finishedLaunching"),
                NSKeyValueObservingOptions::Initial | NSKeyValueObservingOptions::New,
                refcon,
            );
        }
    } else {
        debug!(
            "{}: could not subscribe to finished launching changes for {} ({})\n",
            "workspace_application_observe_finished_launching",
            process.name,
            process.process_id.0
        );
    }
}

pub(crate) fn workspace_application_observe_activation_policy(
    context: &WorkspaceContext,
    process: &Arc<Process>,
) {
    let application = process.ns_application.load(Ordering::Relaxed);
    if let Some(application) = unsafe { application.cast::<NSRunningApplication>().as_ref() } {
        let refcon = Arc::into_raw(Arc::clone(process)) as *mut c_void;
        unsafe {
            application.addObserver_forKeyPath_options_context(
                context,
                &NSString::from_str("activationPolicy"),
                NSKeyValueObservingOptions::Initial | NSKeyValueObservingOptions::New,
                refcon,
            );
        }
    } else {
        debug!(
            "{}: could not subscribe to activation policy changes for {} ({})\n",
            "workspace_application_observe_activation_policy",
            process.name,
            process.process_id.0
        );
    }
}

pub(crate) fn workspace_application_unobserve(
    workspace_context: &WorkspaceContext,
    process: &Arc<Process>,
) {
    let application = process.ns_application.load(Ordering::Relaxed);
    if let Some(application) = unsafe { application.cast::<NSRunningApplication>().as_ref() } {
        if remove_observer_swallowing_exception(
            application,
            workspace_context,
            &NSString::from_str("activationPolicy"),
            process,
        ) {
            release_kvo_refcon_on_main_queue(process);
        }

        if remove_observer_swallowing_exception(
            application,
            workspace_context,
            &NSString::from_str("finishedLaunching"),
            process,
        ) {
            release_kvo_refcon_on_main_queue(process);
        }
    }
}

pub(crate) fn remove_observer_swallowing_exception(
    application: &NSRunningApplication,
    workspace_context: &WorkspaceContext,
    key_path: &NSString,
    process: &Process,
) -> bool {
    let context = (process as *const Process).cast_mut().cast::<c_void>();
    objc2::exception::catch(AssertUnwindSafe(|| unsafe {
        application.removeObserver_forKeyPath_context(workspace_context, key_path, context);
    }))
    .is_ok()
}

pub(crate) fn release_kvo_refcon_on_main_queue(process: &Process) {
    let refcon = process as *const Process;
    dispatch_after_on_main_queue(0, move || drop(unsafe { Arc::from_raw(refcon) }));
}
