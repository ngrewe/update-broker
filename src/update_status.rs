use std::ops::Deref;
use std::path::Path;
use std::time::SystemTime;
use inotify::wrapper::Event;
static VERSION_NULL: &'static str = "0.0.0";

#[derive(Debug,PartialEq,Eq)]
pub enum UpdateStatus {
    Idle,
    CheckingForUpdate,
    UpdateAvailable,
    Downloading,
    Verifying,
    Finalizing,
    UpdatedNeedReboot,
    ReportingErrorEvent
}

impl Deref for UpdateStatus {
    type Target = str;

    fn deref(&self) -> &'static str {
        match *self {
            UpdateStatus::Idle => &"UPDATE_STATUS_IDLE",
            UpdateStatus::CheckingForUpdate => &"UPDATE_STATUS_CHECKING_FOR_UPDATE",
            UpdateStatus::UpdateAvailable => &"UPDATE_STATUS_UPDATE_AVAILABLE",
            UpdateStatus::Downloading => &"UPDATE_STATUS_DOWNLOADING",
            UpdateStatus::Verifying => &"UPDATE_STATUS_VERIFYING",
            UpdateStatus::Finalizing => &"UPDATE_STATUS_FINALIZING",
            UpdateStatus::UpdatedNeedReboot => &"UPDATE_STATUS_UPDATED_NEED_REBOOT",
            UpdateStatus::ReportingErrorEvent => &"UPDATE_STATUS_REPORTING_ERROR_EVENT"
        }
    }
}

#[derive(Debug)]
pub struct UpdateStatusIndication<'a> {
    last_checked_time: SystemTime,
    progress: f64,
    current_operation: UpdateStatus,
    new_version: &'a str,
    new_payload_size: i64
}

impl <'a> UpdateStatusIndication<'a> {
    fn new(current_operation: UpdateStatus) -> UpdateStatusIndication<'static> {
        UpdateStatusIndication{
            last_checked_time: SystemTime::now(),
            progress: 0.0,
            current_operation: current_operation,
            new_version: VERSION_NULL,
            new_payload_size: 0
        }
    }

    pub fn from_inotify_event(event: &Event) -> Option<UpdateStatusIndication<'static>> {
        let status: Option<UpdateStatus>;
        if event.is_create() {
            status = Option::Some(UpdateStatus::UpdatedNeedReboot);
        } else if event.is_delete() {
            status = Option::Some(UpdateStatus::Idle);
        } else {
            status = Option::None;
        }
        status.map(|s| UpdateStatusIndication::new(s))
    }
}

