#![feature(unboxed_closures)]
#![feature(fn_traits)]
#![feature(fnbox)]

#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_journald;
extern crate slog_async;
extern crate libc;
extern crate futures;
extern crate tokio_core;
extern crate tokio_inotify;
extern crate tokio_signal;
extern crate tokio_timer;
extern crate dbus_tokio;
extern crate dbus;
extern crate inotify;
use slog::*;
use slog_journald::JournaldDrain;

use std::{thread, time};
use std::path::Path;
use std::rc::Rc;

use futures::stream::Stream;
use futures::Future;
use tokio_inotify::AsyncINotify;
use tokio_core::reactor::Core;
use tokio_timer::Timer;

mod apt;
mod update_status;
mod server;

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
    let reboot_sentinel = Path::new(SENTINEL_FILE);
    let logger = Rc::new(root);
    if let Err(e) = server::engine(&reboot_sentinel, logger.clone()) {
        error!(&logger, "Startup failure. {:?}", e);
        let timer = Timer::default();
        timer.sleep(time::Duration::from_millis(200)).wait().unwrap();
        std::process::exit(1);
    } else {
        info!(&logger, "Shutdown");
    }
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

