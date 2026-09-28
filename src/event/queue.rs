use std::os::unix::net::UnixStream;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

use crate::ffi::accessibility::AXUIElement;
use crate::ffi::core_foundation::CFRetainedAssumedSendAndSync;
use crate::ffi::core_graphics::CGEvent;
use crate::mouse::tap::MouseModifier;
use crate::process::model::Process;
use crate::support::color::RgbaColor;
use crate::support::handles::{DisplayId, ProcessId, SpaceId, WindowId};

pub(crate) enum Event {
    ApplicationLaunched(Arc<Process>),
    ApplicationTerminated(Arc<Process>),
    ApplicationFrontSwitched(Arc<Process>),
    ApplicationVisible(ProcessId),
    ApplicationHidden(ProcessId),
    WindowCreated(CFRetainedAssumedSendAndSync<AXUIElement>),
    WindowDestroyed(WindowId),
    WindowFocused(WindowId),
    WindowMoved(WindowId),
    WindowResized(WindowId),
    WindowMinimized(WindowId),
    WindowDeminimized(WindowId),
    WindowTitleChanged(WindowId),
    SkylightWindowOrdered(WindowId),
    SkylightWindowDestroyed(WindowId),
    SkylightSpaceCreated(SpaceId),
    SkylightSpaceDestroyed(SpaceId),
    SpaceChanged,
    DisplayAdded(DisplayId),
    DisplayRemoved(DisplayId),
    DisplayMoved(DisplayId),
    DisplayResized(DisplayId),
    DisplayChanged,
    MouseDown {
        event: CFRetainedAssumedSendAndSync<CGEvent>,
        event_modifier: MouseModifier,
    },
    MouseUp {
        event: CFRetainedAssumedSendAndSync<CGEvent>,
    },
    MouseDragged {
        event: CFRetainedAssumedSendAndSync<CGEvent>,
    },
    MouseMoved {
        event: CFRetainedAssumedSendAndSync<CGEvent>,
        event_modifier: MouseModifier,
    },
    MissionControlShowAllWindows,
    MissionControlShowFrontWindows,
    MissionControlShowDesktop,
    MissionControlEnter,
    MissionControlCheckForExit,
    MissionControlExit,
    DockDidRestart,
    MenuOpened(WindowId),
    MenuClosed,
    MenuBarHiddenChanged,
    DockDidChangePreferences,
    SystemWoke,
    SystemAccentColorChanged(RgbaColor),
    InsertFeedbackFadeInStep,
    DaemonMessage(UnixStream),
}

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

pub(crate) fn post_event_to_event_loop(event: Event) {
    if let Some(event_sender) = EVENT_SENDER.get() {
        let _ = event_sender.send(event);
    }
}

pub(crate) fn create_event_loop_channel_storing_its_sender() -> Receiver<Event> {
    let (event_sender, event_receiver) = channel::<Event>();
    let _ = EVENT_SENDER.set(event_sender);

    event_receiver
}
