use dbus::tree::{MethodErr, Signal};
use dbus::{BusType, Connection, NameFlag};
use dbus_tokio::tree::{AFactory, ATree, ATreeServer};
use dbus_tokio::AConnection;
use futures::future::Executor;
use futures::Future;
use futures::Stream;
use std::path::Path;
use std::result::Result;
use tokio_core::reactor::Core;
use tokio_signal::unix::{Signal as USignal, SIGTERM};
use tokio_timer::Timer;

use slog::Logger;
use std::io::{Error as IoError, ErrorKind};
use std::process;
use std::rc::Rc;
use std::sync::Arc;
use std::time;

use update_status::{UpdateStatusIndication, UpdateStatusIndicationConsumer, UpdateStatusNotifier};

struct DBusUpdateIndicator {
    connection: Rc<Connection>,
    signal: Arc<Signal<ATree<()>>>,
    logger: Rc<Logger>,
}

impl UpdateStatusIndicationConsumer for DBusUpdateIndicator {
    fn status_changed(&self, status: UpdateStatusIndication) {
        info!(&(self.logger), "Broadcasting update status: {:?}", status);
        self.connection
            .send(
                self.signal
                    .msg(
                        &"/com/coreos/update1".into(),
                        &"com.coreos.update1.Manager".into(),
                    ).append1(status.last_checked_time_millis())
                    .append1(status.progress)
                    .append2::<&str, &str>(&status.current_operation, &status.new_version)
                    .append1(status.new_size),
            ).map(|_| ())
            .unwrap_or_else(|e| {
                warn!(&self.logger, "Could not broadcast update signal. {:?}", e);
            });
    }
}

pub fn engine(path: &Path, logger: Rc<Logger>) -> Result<(), IoError> {
    let owned_path = path.to_owned();
    let connection_r = Connection::get_private(BusType::System);
    if connection_r.is_err() {
        return connection_r.map(|_| ()).map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!("Error creating connection. {:?}", e),
            )
        });
    }
    let connection = Rc::new(connection_r.unwrap());
    let registration =
        connection.register_name("com.coreos.update1", NameFlag::ReplaceExisting as u32);
    if registration.is_err() {
        return registration.map(|_| ()).map_err(|e| {
            IoError::new(ErrorKind::Other, format!("Error registering name. {:?}", e))
        });
    }
    let f = AFactory::new_afn::<()>();

    let signal = Arc::new(
        f.signal("StatusUpdate", ())
            .sarg::<i64, _>("last_checked_time")
            .sarg::<f64, _>("progress")
            .sarg::<&str, _>("current_operation")
            .sarg::<&str, _>("new_version")
            .sarg::<i64, _>("new_size"),
    );
    let l = logger.clone();
    let l2 = logger.clone();
    let l3 = logger.clone();
    let tree = f.tree(ATree::new()).add(
        f.object_path("/com/coreos/update1", ())
            .introspectable()
            .add(
                f.interface("com.coreos.update1.Manager", ())
                    .add_s(signal.clone())
                    .add_m(
                        f.method("GetStatus", (), move |m| {
                            let status = UpdateStatusIndication::from_path(&owned_path);
                            debug!(
                                &l,
                                "Sending update status to {:?}: {:?}",
                                m.msg.sender(),
                                status
                            );
                            Ok(vec![
                                m.msg
                                    .method_return()
                                    .append1(status.last_checked_time_millis())
                                    .append1(status.progress)
                                    .append2::<&str, &str>(
                                        &status.current_operation,
                                        &status.new_version,
                                    ).append1(status.new_size),
                            ])
                        }).outarg::<i64, _>("last_checked_time")
                        .outarg::<f64, _>("progress")
                        .outarg::<&str, _>("current_operation")
                        .outarg::<&str, _>("new_version")
                        .outarg::<i64, _>("new_size"),
                    ).add_m(f.method("AttemptUpdate", (), move |_| {
                        warn!(&l2, "Ignoring attempt to call AttemptUpdate");
                        Err(MethodErr::failed(&"Not implemented".to_owned()))
                    })).add_m(f.method("ResetStatus", (), move |_| {
                        warn!(&l3, "Ignoring attempt to call ResetStatus");
                        Err(MethodErr::failed(&"Not implemented".to_owned()))
                    })),
            ),
    );

    let registration2_r = tree.set_registered(&connection, true);
    if registration2_r.is_err() {
        return registration2_r.map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!("Error registering D-Bus tree. {:?}", e),
            )
        });
    }
    let core_r = Core::new();
    if core_r.is_err() {
        return core_r.map(|_| ());
    }
    let mut core = Core::new().unwrap();
    let aconn = AConnection::new(connection.clone(), core.handle()).unwrap();
    let server = ATreeServer::new(connection.clone(), &tree, aconn.messages().unwrap());

    // Make the server run forever
    let server = server.for_each(|m| {
        debug!(&logger, "Unhandled message: {:?}", m);
        Ok(())
    });

    let notifier = UpdateStatusNotifier::new_with_path_and_consumer(
        &core.handle(),
        path,
        Box::new(DBusUpdateIndicator {
            connection: connection,
            signal: signal.clone(),
            logger: logger.clone(),
        }),
        logger.clone(),
    );

    if notifier.is_err() {
        return notifier.map(|_| ());
    }

    let l4 = logger.clone();
    info!(&logger, "Monitoring {:?}", path);
    let ex = core.execute(notifier.unwrap().map_err(move |e| {
        error!(&l4, "File watch task exited. {:?}", e);
        Timer::default()
            .sleep(time::Duration::from_millis(200))
            .wait()
            .unwrap();
        process::exit(1);
    }));
    if ex.is_err() {
        return ex.map(|_| ()).map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!("Could not schedule inotify watcher. {:?}", e),
            )
        });
    }
    let termination = USignal::new(SIGTERM, &core.handle())
        .flatten_stream()
        .take(1)
        .map_err(|_| ())
        .into_future()
        .select2(server.map_err(|_| ()));
    return core
        .run(termination)
        .map(|_| ())
        .map_err(|_| IoError::new(ErrorKind::Other, "Error running server"));
    // Ok(())
}
