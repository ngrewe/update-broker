#![feature(unboxed_closures)]
#![feature(fn_traits)]

#[macro_use]
extern crate clap;
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

use std::time;
use std::path::Path;
use std::rc::Rc;

use futures::Future;
use tokio_timer::Timer;
use clap::App;
mod update_status;
mod server;

static SENTINEL_FILE: &'static str = "/var/run/reboot-required";

fn main() {
    let yaml = load_yaml!("cli.yaml");
    let matches = App::from_yaml(yaml).get_matches();
    let journal_drain = JournaldDrain.ignore_res();
    let root: Logger;

    if matches.is_present("debug") {
        let decorator = slog_term::TermDecorator::new().build();
        let term_drain = slog_term::FullFormat::new(decorator).build().fuse();
        let term_drain = slog_async::Async::new(term_drain).build().fuse();
        root = Logger::root(Duplicate::new(term_drain, journal_drain).fuse(), o!());
    } else {
        root = Logger::root(journal_drain, o!());
    }
    let reboot_sentinel = Path::new(matches.value_of("file").unwrap_or(SENTINEL_FILE));
    let logger = Rc::new(root);
    if let Err(e) = server::engine(&reboot_sentinel, logger.clone()) {
        error!(&logger, "Startup failure. {:?}", e);
        let timer = Timer::default();
        timer
            .sleep(time::Duration::from_millis(200))
            .wait()
            .unwrap();
        std::process::exit(1);
    } else {
        info!(&logger, "Shutdown");
    }
}
