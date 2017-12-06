#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_journald;
extern crate slog_async;
extern crate libc;
extern crate futures;
extern crate tokio_inotify;
extern crate tokio_core;
extern crate inotify;
use slog::*;
use slog_journald::JournaldDrain;

use std::{thread, time};
use std::path::Path;

use futures::stream::Stream;
use tokio_inotify::AsyncINotify;
use tokio_core::reactor::Core;

mod apt;
mod update_status;


static SENTINEL_FILE: &'static str = "/var/run/reboot-required";


static LOCK_FILE: &'static str = "/var/run/unattended-upgrades.lock";

fn main() {
    let decorator = slog_term::TermDecorator::new().build();
    let drain1 = slog_term::FullFormat::new(decorator).build().fuse();
    let drain1 = slog_async::Async::new(drain1).build().fuse();
    let drain2 = JournaldDrain.ignore_res();
    let root = Logger::root(Duplicate::new(
        drain1, drain2
    ).fuse()
    , o!());
    let mut evloop = Core::new().unwrap();

    let inot = AsyncINotify::init(&evloop.handle()).unwrap();
    let reboot_sentinel = Path::new(SENTINEL_FILE);
    inot.add_watch(reboot_sentinel.parent().unwrap(),
                   tokio_inotify::IN_CREATE | tokio_inotify::IN_DELETE | tokio_inotify::IN_MODIFY)
        .unwrap();

    let show_events = inot.filter(|ev| ev.name.as_os_str() == reboot_sentinel.file_name().unwrap()).map(|ev| update_status::UpdateStatusIndication::from_inotify_event(&ev)).for_each(|ev| {
            info!(root, "update {:?}", ev);
        Ok(())
    });

    evloop.run(show_events).unwrap();
    // update(root, "user");
}

fn get_allowed_origins() -> Vec<String> {
    vec!(String::from("Ubuntu:xenial-security"))
}

fn get_blacklisted_pkgs() -> Vec<String> {
    vec!(String::from("kernel"))
}

fn get_whitelisted_pkgs() -> Vec<String> {
    vec!(String::from("volatile-pkg"))
}

fn update(root_logger: Logger, initiator: &str) -> Result<()> {

    let allowed_origins = get_allowed_origins();
    let blacklisted_pkgs = get_blacklisted_pkgs();
    let whitelisted_pkgs = get_whitelisted_pkgs();
    let logger = root_logger.new(o!("initiator" => String::from(initiator)));
    info!(logger, "Initial blacklisted packages: {:?}", blacklisted_pkgs);
    info!(logger, "Initial whitelisted packages: {:?}", whitelisted_pkgs);
    info!(logger, "Starting unattended upgrades");
    let lock_file = apt::AptFileLock(LOCK_FILE);
    if let Ok(guard) = lock_file.lock() {
        let mut updater = Updater{
            logger: logger,
            guard: guard,
            allowed_origins: allowed_origins,
            blacklisted_pkgs: blacklisted_pkgs,
            whitelisted_pkgs: whitelisted_pkgs
        };
        updater.update_under_lock()
    } else {
        if let Some(msg) = apt::last_error_consuming() {
            error!(logger,"No lock: {}", msg);
        } else {
            error!(logger, "Unkown error acquring lock");
        }
    }
    Ok(())
}

struct Updater {
    guard: apt::AptFileLockGuard,
    logger: Logger,
    allowed_origins: Vec<String>,
    whitelisted_pkgs: Vec<String>,
    blacklisted_pkgs: Vec<String>
}

impl Updater {
    fn update_under_lock(&mut self) {
        info!(self.logger, "Allowed origins are: {:?}", self.allowed_origins);
        let start = time::Instant::now();
    }
}
