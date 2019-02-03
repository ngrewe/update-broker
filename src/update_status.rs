// This file is part of the upgrade broker
// (c) 2017 FutureTV Production GmbH, Niels Grewe
use futures::Future;
use futures::Stream;
use inotify::wrapper::Event;
use slog::Logger;
use std::ffi::OsString;
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::ops::Deref;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_core::reactor::Handle;
use tokio_inotify::AsyncINotify;
use tokio_inotify::{IN_CREATE, IN_DELETE};

static VERSION_ZERO: &'static str = "0.0.0";

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum UpdateStatus {
    Idle,
    CheckingForUpdate,
    UpdateAvailable,
    Downloading,
    Verifying,
    Finalizing,
    UpdatedNeedReboot,
    ReportingErrorEvent,
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
            UpdateStatus::ReportingErrorEvent => &"UPDATE_STATUS_REPORTING_ERROR_EVENT",
        }
    }
}

#[derive(Debug)]
pub struct UpdateStatusIndication {
    pub last_checked_time: SystemTime,
    pub progress: f64,
    pub current_operation: UpdateStatus,
    pub new_version: String,
    pub new_size: i64,
}

impl UpdateStatusIndication {
    fn new(current_operation: UpdateStatus) -> UpdateStatusIndication {
        UpdateStatusIndication {
            last_checked_time: SystemTime::now(),
            progress: 0.0,
            current_operation: current_operation,
            new_version: UpdateStatusIndication::version(),
            new_size: 0,
        }
    }

    fn version() -> String {
        Command::new("lsb_release")
            .arg("-r")
            .arg("-s")
            .output()
            .and_then(|o| {
                String::from_utf8(o.stdout)
                    .map(|mut s| {
                        let len = s.trim_right().len();
                        s.truncate(len);
                        s
                    }).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid version string"))
            }).unwrap_or_else(|_| String::from(VERSION_ZERO))
    }

    pub fn last_checked_time_millis(&self) -> i64 {
        let duration = self.last_checked_time.duration_since(UNIX_EPOCH).unwrap();
        let nanos = duration.subsec_nanos() as u64;
        return ((1000 * 1000 * 1000 * duration.as_secs() + nanos) / (1000 * 1000)) as i64;
    }
    pub fn from_inotify_event(event: &Event) -> Option<UpdateStatusIndication> {
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

    pub fn from_path(path: &Path) -> UpdateStatusIndication {
        let status: UpdateStatus;
        if path.exists() {
            status = UpdateStatus::UpdatedNeedReboot;
        } else {
            status = UpdateStatus::Idle;
        }
        UpdateStatusIndication::new(status)
    }
}

#[derive(Debug)]
pub struct UpdateStatusNotifier();

pub trait UpdateStatusIndicationConsumer {
    fn status_changed(&self, status: UpdateStatusIndication) -> ();
}

impl UpdateStatusNotifier {
    fn add_watch(inotify: AsyncINotify, path: &Path) -> Result<AsyncINotify> {
        path.parent().map_or(
            Err(Error::new(
                ErrorKind::NotFound,
                "Invalid path to reboot sentinel file",
            )),
            |dir| {
                inotify
                    .add_watch(dir, IN_CREATE | IN_DELETE)
                    .map(|_| inotify)
            },
        )
    }

    fn get_file_name_os_string(path: &Path) -> Option<OsString> {
        path.file_name().map(|f| f.to_os_string())
    }

    pub fn new_with_path_and_consumer(
        handle: &Handle,
        path: &Path,
        consumer: Box<UpdateStatusIndicationConsumer>,
        logger: Rc<Logger>,
    ) -> Result<Box<Future<Item = (), Error = Error>>> {
        if let Some(sentinel_file) = UpdateStatusNotifier::get_file_name_os_string(path) {
            AsyncINotify::init(handle)
                .and_then(|stream| UpdateStatusNotifier::add_watch(stream, path))
                .map(|stream| {
                    stream.filter(move |event: &Event| event.name.as_os_str() == sentinel_file)
                }).map(|stream| {
                    stream
                        .map(|ev| UpdateStatusIndication::from_inotify_event(&ev))
                        .map_err(move |e| {
                            warn!(&logger, "Error handling watch. {:?}", e);
                            e
                        })
                }).map(|stream| {
                    return Box::new(stream.for_each(move |v| {
                        if let Some(indication) = v {
                            consumer.status_changed(indication)
                        }
                        Ok(())
                    })) as Box<Future<Item = (), Error = Error>>;
                })
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                "Invalid path to reboot sentinel file",
            ))
        }
    }
}
