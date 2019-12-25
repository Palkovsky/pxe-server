use dhcp::{DHCPDgram, DHCPDgramBuilder};
use pxe::{PXEBuilder};

use std::net::{SocketAddr, SocketAddrV4, UdpSocket, Ipv4Addr};
use std::io::ErrorKind;

const ADDR: &'static str = "192.168.1.103:67";

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
    let socket = UdpSocket::bind(ADDR)?;
    socket.set_broadcast(true).expect("set_broadcast call failed");

    println!("Listening on {}...", ADDR);

    let BROADCAST = SocketAddrV4::new(
        Ipv4Addr::new(255, 255, 255, 255), 68
    );

    loop {
        let (dhcp, from) = listen(&socket);
        let dhcp = dhcp.swap_endianess();
        let body = dhcp.body;

        if body.op != BOOT_REQUEST {
            continue;
        }

        let res = match dhcp.option(MESSAGE_TYPE) {
            Some(&[DISCOVER]) => {
                println!("DISCOVER FROM: {}", from);
                discover(dhcp)
            },
            Some(&[REQUEST]) => {
                println!("REQUEST FROM: {}", from);
                None
            },
            _ => None
        };

        res.map(|res| {
            println!("RESPONDED");
            println!("{}", res);

            let res = res.swap_endianess();
            socket.send_to(&res.as_bytes()[..], &BROADCAST);
        });
    }
}

fn discover(dhcp: DHCPDgram) -> Option<DHCPDgram> {
    println!("{}", dhcp);

    let copy_string = |string: &str, target: &mut [u8]| {
        let zipped = target.into_iter().zip(string.as_bytes().iter());
        for (place, data) in zipped {
            *place = *data
        }
    };

    let mut body = dhcp.body;

    body.op = BOOT_REPLY;
    copy_string("PXEServer", &mut body.sname);
    copy_string("memtest_x86.0", &mut body.filename);

    let pxe = PXEBuilder::default()
        .start()
        .boot_servers(vec![ Ipv4Addr::new(192, 168, 1, 103) ])
        .end()
        .build();

    let maybe_client_id = dhcp.option(CLIENT_MAC);
    if maybe_client_id.is_none() {
        return None;
    }
    let client_id = maybe_client_id.unwrap();

    DHCPDgramBuilder::default()
        .body(body)
        .option(MESSAGE_TYPE, &[OFFER])
        .option(SERVER_ID, &[192, 168, 1, 103])
        .option(CLIENT_MAC, client_id)
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
                    Err(std::io::Error::new(ErrorKind::AddrNotAvailable, "IPv4 only."))
                }
            })
            // Convert bytes to DHCP datagram
            .and_then(|(amt, ipv4)| {
                let err = std::io::Error::new(
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
