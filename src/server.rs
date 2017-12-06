use dbus::{Connection, BusType, NameFlag, Error};
use dbus::tree::MethodErr;
use dbus_tokio::tree::{AFactory, ATree, ATreeServer};
use dbus_tokio::AConnection;
use std::result::Result;
use std::path::Path;
use tokio_core::reactor::Core;
use futures::Stream;

use std::sync::Arc;
use std::rc::Rc;


use update_status::UpdateStatusIndication;

pub fn engine(path: &Path) -> Result<(),Error> {
    let owned_path = path.to_owned();
    let connection_r = Connection::get_private(BusType::System);
    if connection_r.is_err() {
        return connection_r.map(|_| ())
    }
    let connection = Rc::new(connection_r.unwrap());
    let registration = connection.register_name("com.coreos.update1", NameFlag::ReplaceExisting as u32);
    if registration.is_err() {
        return registration.map(|_| ())
    }
    let f = AFactory::new_afn::<()>();

    let signal = Arc::new(f.signal("StatusUpdate", ())
    .sarg::<i64,_>("last_checked_time")
    .sarg::<f64,_>("progress")
    .sarg::<&str,_>("current_operation")
    .sarg::<&str,_>("new_version")
    .sarg::<i64,_>("new_size"));

    let tree = f.tree(ATree::new()).add(f.object_path("/com/coreos/update1/Manager", ()).introspectable().add(
        f.interface("com.coreos.update1.Manager", ())
            .add_s(signal)
            .add_m(f.method("GetStatus", (), move |m| {
                let mret = m.msg.method_return();
                let status = UpdateStatusIndication::from_path(&owned_path);
                Ok(vec!(m.msg.method_return().append1(status.last_checked_time_millis())
                    .append1(status.progress)
                    .append2::<&str,&str>(&status.current_operation, &status.new_version)
                    .append1(status.new_size)))
            }).outarg::<i64,_>("last_checked_time")
             .outarg::<f64,_>("progress")
             .outarg::<&str,_>("current_operation")
             .outarg::<&str,_>("new_version")
             .outarg::<i64,_>("new_size")
        )
    ));

    let registration2_r = tree.set_registered(&connection, true);
    if registration2_r.is_err() {
        return registration2_r;
    }
    let mut core = Core::new().unwrap();
    let aconn = AConnection::new(connection.clone(), core.handle()).unwrap();
    let server = ATreeServer::new(connection.clone(), &tree, aconn.messages().unwrap());
    
    // Make the server run forever
    let server = server.for_each(|m| { println!("Unhandled message: {:?}", m); Ok(()) });
    core.run(server).unwrap(); 
    Ok(())
}
