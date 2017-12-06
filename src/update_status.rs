use std::ops::Deref;
use std::path::Path;
use std::time::{Duration,UNIX_EPOCH,SystemTime};
use std::cell::RefCell;
use std::io::Result;
use std::io::{Error, ErrorKind};
use inotify::wrapper::Event;
use futures::stream::{Filter,ForEach};
use futures::Stream;
use std::marker::Sized;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt::Debug;
use futures::Future;
use tokio_core::reactor::Handle;
use tokio_inotify::AsyncINotify;
use std::boxed::FnBox;
use tokio_inotify::{IN_CREATE, IN_DELETE};
use std::process::Command;

static VERSION_ZERO: &'static str = "0.0.0";

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
pub struct UpdateStatusIndication {
    pub last_checked_time: SystemTime,
    pub progress: f64,
    pub current_operation: UpdateStatus,
    pub new_version: String,
    pub new_size: i64
}

impl UpdateStatusIndication {
    fn new(current_operation: UpdateStatus) -> UpdateStatusIndication {
        UpdateStatusIndication{
            last_checked_time: SystemTime::now(),
            progress: 0.0,
            current_operation: current_operation,
            new_version: UpdateStatusIndication::version(),
            new_size: 0
        }
    }

    fn version() -> String {
        Command::new("lsb_release")
        .arg("-r")
        .arg("-s")
        .output()
        .and_then(|o| String::from_utf8(o.stdout)
            .map(|mut s| {
                let len = s.trim_right().len();
                s.truncate(len);
                s
            })
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid version string"))
        ).unwrap_or_else(|_| String::from(VERSION_ZERO))
    }

    pub fn last_checked_time_millis(&self) -> i64 {
        let duration = self.last_checked_time.duration_since(UNIX_EPOCH).unwrap();
        let nanos = duration.subsec_nanos() as u64;
        return ((1000*1000*1000 * duration.as_secs() + nanos)/(1000 * 1000)) as i64;
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

// #[derive(Debug)]
pub struct UpdateStatusNotifier();

#[derive(Debug)]
struct FileNameFilter(OsString);

impl <'a> FnOnce<(&'a Event,)> for FileNameFilter {
    type Output = bool;
    extern "rust-call" fn call_once(self, args: (&'a Event,)) -> bool {
        args.0.name.as_os_str() == &(self.0)
    }

}

impl <'a> FnMut<(&'a Event,)> for FileNameFilter {
    extern "rust-call" fn call_mut(&mut self, args: (&'a Event,)) -> bool {
        args.0.name.as_os_str() == &(self.0)
    }
}

pub trait UpdateStatusIndicationConsumer {
    fn status_changed(&self, status: UpdateStatusIndication) -> ();
}

struct UpdateStatusIndicatorAdapter(Box<UpdateStatusIndicationConsumer>);

impl FnMut<(Option<UpdateStatusIndication>,)> for UpdateStatusIndicatorAdapter {
    extern "rust-call" fn call_mut(&mut self, args: (Option<UpdateStatusIndication>,)) -> Result<()> {
        if args.0.is_some() {
            self.0.status_changed(args.0.unwrap());
        }
        Ok(())
    }
}

impl FnOnce<(Option<UpdateStatusIndication>,)> for UpdateStatusIndicatorAdapter {
    type Output = Result<()>;
    extern "rust-call" fn call_once(mut self, args: (Option<UpdateStatusIndication>,)) -> Result<()> {
        if args.0.is_some() {
            self.0.status_changed(args.0.unwrap());
        }
        Ok(())
    }
}



impl UpdateStatusNotifier {
    fn add_watch(inotify: AsyncINotify, path: &Path) -> Result<AsyncINotify> {
        path.parent().map_or(Err(Error::new(ErrorKind::NotFound, "Invalid path to reboot sentinel file")),
        |dir| inotify.add_watch(dir, IN_CREATE | IN_DELETE).map(|_| inotify))
    }

    fn get_filtered(inotify: AsyncINotify, path: &Path) -> Result<Filter<AsyncINotify, FileNameFilter>> {
        if let Some(sentinel_file) = path.file_name() {
            Ok(inotify.filter(FileNameFilter(sentinel_file.to_os_string())))
        } else {
            Err(Error::new(ErrorKind::NotFound, "Invalid path to reboot sentinel file"))
        }
    }

    pub fn new_with_path_and_consumer(handle: &Handle, path: &Path, consume: Box<UpdateStatusIndicationConsumer>) -> Result<Box<Future<Item=(), Error=Error>>> {
        AsyncINotify::init(handle)
            .and_then(|stream| UpdateStatusNotifier::add_watch(stream, path))
            .and_then(|stream| UpdateStatusNotifier::get_filtered(stream, path))
            .map(|filtered| filtered.map(|ev| UpdateStatusIndication::from_inotify_event(&ev)))
            .map(|mapped| {
                return Box::new(mapped.for_each(UpdateStatusIndicatorAdapter(consume))) as Box<Future<Item=(), Error=Error>>
            })
    }
}

