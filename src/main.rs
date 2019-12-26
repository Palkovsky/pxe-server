use dhcp::{DHCPDgram, DHCPDgramBuilder};
use pxe::{PXEBuilder};

use std::{env, io};
use std::io::ErrorKind;
use std::net::{SocketAddr,
               SocketAddrV4,
               UdpSocket,
               Ipv4Addr};

// Operations
const BOOT_REQUEST: u8 = 1;
const BOOT_REPLY: u8 = 2;

// Types
const DISCOVER: u8 = 1;
const OFFER: u8 = 2;
const REQUEST: u8 = 3;
const ACK: u8 = 5;

// Options
const VENDOR_OPTIONS: u8 = 43;
const MESSAGE_TYPE: u8 = 53;
const SERVER_ID: u8 = 54;
const CLASS_ID: u8 = 60;
const CLIENT_MAC: u8 = 97;

fn main() -> std::io::Result<()> {
    // Broadcast, UDP 68. For server responses.
    let broadcast = SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 68);

    // Get server address
    let argv = env::args().collect::<Vec<String>>();
    let addr = argv.get(1)
        .and_then(|addr| addr.parse::<SocketAddrV4>().ok())
        .ok_or({
            let msg = format!("You must specify address and port to listen on. Example: {} x.x.x.x:pp",
                              argv.get(0).unwrap());
            io::Error::new(ErrorKind::InvalidInput, msg)
        })?;

    // Setup socket
    let socket = UdpSocket::bind(&addr)?;
    socket.set_broadcast(true)?;
    println!("Listening on {}...", addr);

    // Main server loop
    loop {
        let (dhcp, from) = listen(&socket);

        // Convert for BE to LE
        let dhcp = dhcp.swap_endianess();
        let body = dhcp.body;

        // Server ignores replies.
        if body.op == BOOT_REPLY {
            continue;
        }

        // Try to create response for request.
        let res = match dhcp.option(MESSAGE_TYPE) {
            Some(&[DISCOVER]) => {
                println!("DHCP_DISCOVER FROM: {}", from);
                discover(&addr, dhcp)
            },
            Some(&[REQUEST]) => {
                println!("DHCP_REQUEST FROM: {}", from);
                None
            },
            _ => {
                println!("UNKNOWN FROM: {}", from);
                None
            }
        };

        // If managed to create response, try to broadcast it.
        match res {
            Some(res) => {
                let res = res.swap_endianess();
                let bytes = res.as_bytes();

                // check
                let _ = socket.send_to(bytes.as_slice(), &broadcast);
                println!("Response sent");
            },
            _ => {
                println!("Unable to create responce.");
            }
        }
    }
}

fn discover(addr: &SocketAddrV4, dhcp: DHCPDgram) -> Option<DHCPDgram> {
    let copy_string = |string: &str, target: &mut [u8]| {
        let zipped = target.into_iter().zip(string.as_bytes().iter());
        for (place, data) in zipped {
            *place = *data
        }
    };

    let mut body = dhcp.body;
    body.op = BOOT_REPLY;
    copy_string("PXEServer", &mut body.sname);
    copy_string("pxelinux.0", &mut body.filename);

    let pxe = PXEBuilder::default()
        .start(false)
        .boot_servers(vec![addr.ip()])
        .end()
        .build();

    DHCPDgramBuilder::default()
        .body(body)
        .option(MESSAGE_TYPE, &[OFFER])
        .option(SERVER_ID, &addr.ip().octets())
        .option(CLASS_ID, "PXEClient".as_bytes())
        .option(VENDOR_OPTIONS, &pxe[..])
        .end()
        .build()
}

// Wait until DHCPDgram received
fn listen(socket: &UdpSocket) -> (DHCPDgram, SocketAddrV4) {
    let mut buf = [0; 1<<12];

    loop {
        let maybe_dhcp = socket.recv_from(&mut buf)
            // Convert SockAddr to SocketAddrV4
            .and_then(|(amt, from)| {
                if let SocketAddr::V4(ipv4) = from {
                    Ok((amt, ipv4))
                } else {
                    Err(io::Error::new(ErrorKind::AddrNotAvailable, "IPv4 only."))
                }
            })
            // Convert bytes to DHCP datagram
            .and_then(|(amt, ipv4)| {
                let err = io::Error::new(
                    ErrorKind::InvalidData,
                    "Unable to interpret as DHCP datagram."
                );
                DHCPDgram::from_bytes(&buf[..amt])
                    .ok_or(err)
                    .map(|dhcp| (dhcp, ipv4))
            })
            // Convert to Option<T>
            .ok();

        // If failed, keep listening.
        if maybe_dhcp.is_some() {
            return maybe_dhcp.unwrap();
        }
    }
}
