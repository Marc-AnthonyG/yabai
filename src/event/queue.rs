use std::os::unix::net::UnixStream;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

use crate::ffi::accessibility::AXUIElement;
use crate::ffi::core_foundation::SendCFRetained;
use crate::ffi::core_graphics::CGEvent;
use crate::mouse::tap::MouseMod;
use crate::process::model::Process;
use crate::support::color::RgbaColor;
use crate::support::handles::{DisplayId, ProcessId, SpaceId, WindowId};

pub(crate) enum Event {
    ApplicationLaunched(Arc<Process>),
    ApplicationTerminated(Arc<Process>),
    ApplicationFrontSwitched(Arc<Process>),
    ApplicationVisible(ProcessId),
    ApplicationHidden(ProcessId),
    WindowCreated(SendCFRetained<AXUIElement>),
    WindowDestroyed(WindowId),
    WindowFocused(WindowId),
    WindowMoved(WindowId),
    WindowResized(WindowId),
    WindowMinimized(WindowId),
    WindowDeminimized(WindowId),
    WindowTitleChanged(WindowId),
    SlsWindowOrdered(WindowId),
    SlsWindowDestroyed(WindowId),
    SlsSpaceCreated(SpaceId),
    SlsSpaceDestroyed(SpaceId),
    SpaceChanged,
    DisplayAdded(DisplayId),
    DisplayRemoved(DisplayId),
    DisplayMoved(DisplayId),
    DisplayResized(DisplayId),
    DisplayChanged,
    MouseDown {
        event: SendCFRetained<CGEvent>,
        event_modifier: MouseMod,
    },
    MouseUp {
        event: SendCFRetained<CGEvent>,
    },
    MouseDragged {
        event: SendCFRetained<CGEvent>,
    },
    MouseMoved {
        event: SendCFRetained<CGEvent>,
        event_modifier: MouseMod,
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
    DockDidChangePref,
    SystemWoke,
    SystemAccentColorChanged(RgbaColor),
    InsertFeedbackFadeInStep,
    DaemonMessage(UnixStream),
}

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

pub(crate) fn event_loop_post(event: Event) {
    if let Some(event_sender) = EVENT_SENDER.get() {
        let _ = event_sender.send(event);
    }
}

pub(crate) fn event_loop_begin() -> Receiver<Event> {
    let (event_sender, event_receiver) = channel::<Event>();
    let _ = EVENT_SENDER.set(event_sender);

    event_receiver
}
