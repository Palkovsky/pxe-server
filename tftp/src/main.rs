use tftp::TFTPServer;
use std::net::SocketAddrV4;

const ADDR: &'static str = "192.168.1.104:69";

fn main() {
    let addr = ADDR.parse::<SocketAddrV4>().unwrap();
    TFTPServer::new()
        .start(addr);
}
