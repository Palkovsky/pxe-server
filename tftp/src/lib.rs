use std::io;
use std::io::Read;

use std::fs::File;
use std::net::{
    UdpSocket,
    SocketAddr,
    SocketAddrV4
};
use std::collections::HashMap;

struct TFTPTransfer {
    block_cnt: u16,
    block_sz: u16,
    done: bool,
    file: File
}

impl TFTPTransfer {
    fn next_block(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }

        // Read new block from file.
        let mut buff = vec![0; self.block_sz as usize];
        let read = self.file.read(&mut buff);
        if read.is_err() {
            return None;
        }

        let read = read.unwrap();

        self.block_cnt += 1;
        self.done = read as u16 != self.block_sz;
        Some(buff.to_vec())
    }
}

pub struct TFTPServer {
    transfers: HashMap<SocketAddr, TFTPTransfer>
}

impl TFTPServer {
    pub fn new() -> Self {
        Self {
            transfers: HashMap::new()
        }
    }

    pub fn start(&mut self, addr: SocketAddrV4) -> io::Result<()> {
        let socket = UdpSocket::bind(addr)?;
        loop {
            let _ = self.listen(&socket);
        }
    }

    fn send_next(&mut self, to: &SocketAddr) -> io::Result<Vec<u8>> {
        let is_done = self.transfers.get_mut(&to).map(|t| t.done).unwrap_or(false);
        if is_done {
            self.transfers.remove(to);
            return Err(io::Error::new(io::ErrorKind::ConnectionRefused, "Transfer finished."));
        }

        self.transfers.get_mut(&to)
            .and_then(|transfer| transfer.next_block().map(|blk| (transfer, blk)))
            .map(|(transfer, bytes)| TFTP::data(transfer.block_cnt, bytes))
            .ok_or(io::Error::new(io::ErrorKind::InvalidData, "Unable to send next block."))
    }

    fn listen(&mut self, socket: &UdpSocket) -> io::Result<()> {
        let mut buf = [0; 4096];
        let (amt, from) = socket.recv_from(&mut buf)?;

        let buf = &mut buf[..amt];
        let res = match buf.get(1) {
            Some(&TFTP::RRQ) => {
                let transfer = TFTPTransfer {
                    block_cnt: 0,
                    block_sz: 1456,
                    done: false,
                    file: File::open("R:\\memtest_x86.0").unwrap()
                };
                self.transfers.insert(from.clone(), transfer);
                Ok(TFTP::opt_ack(0))
            },
            Some(&TFTP::ACK) =>
                self.send_next(&from),
            Some(_) =>
                Err(io::Error::new(io::ErrorKind::InvalidData, "Unsuported operation.")),
            _ =>
                Err(io::Error::new(io::ErrorKind::InvalidData, "Not enought data."))
        }?;

        socket.send_to(res.as_slice(), &from)?;
        Ok(())
    }
}

struct TFTP {}
impl TFTP {

    pub const RRQ: u8 = 1;
    pub const WRQ: u8 = 2;
    pub const DATA: u8 = 3;
    pub const ACK: u8 = 4;
    pub const ERROR: u8 = 5;
    pub const OPT_ACK: u8 = 6;

    // Into<String> should probably be replaced with CStr or IntoCStr trait.
    pub fn wrq(filname: impl Into<String>, mode: impl Into<String>) -> Vec<u8> {
        let filname = filname.into();
        let mode = mode.into();

        let mut filname_bytes = vec![];
        filname_bytes.extend_from_slice(filname.as_bytes());
        filname_bytes.push(0x00);

        let mut mode_bytes = vec![];
        mode_bytes.extend_from_slice(mode.as_bytes());
        mode_bytes.push(0x00);

        vec![vec![Self::WRQ, 0x02], filname_bytes, mode_bytes].concat()
    }

    pub fn rrq(filname: impl Into<String>, mode: impl Into<String>) -> Vec<u8> {
        let mut wrq = Self::wrq(filname, mode);
        wrq[1] = Self::RRQ;
        wrq
    }

    pub fn data(block: u16, bytes: Vec<u8>) -> Vec<u8> {
        let lo = (block & 0xFF) as u8;
        let hi = (block >> 8) as u8;
        vec![vec![0x00, Self::DATA, hi, lo], bytes].concat()
    }

    pub fn ack(block: u16) -> Vec<u8> {
        let lo = (block & 0xFF) as u8;
        let hi = (block >> 8) as u8;
        vec![0x00, Self::ACK, hi, lo]
    }

    pub fn opt_ack(block: u16) -> Vec<u8> {
        let lo = (block & 0xFF) as u8;
        let hi = (block >> 8) as u8;
        let blk_size = vec![0x62, 0x6c, 0x6b, 0x73, 0x69, 0x7a, 0x65, 0x00, 0x31, 0x34, 0x35, 0x36, 0x00];
        vec![vec![0x00, Self::OPT_ACK], blk_size].concat()
    }

    pub fn error(code: u16, msg: impl Into<String>) -> Vec<u8> {
        let msg = msg.into();

        let mut msg_bytes = vec![];
        msg_bytes.extend_from_slice(msg.as_bytes());
        msg_bytes.push(0x00);

        let lo = (code & 0xFF) as u8;
        let hi = (code >> 8) as u8;

        vec![vec![0x00, Self::ERROR, hi, lo], msg_bytes].concat()
    }
}

#[test]
fn wrq_rrq_test() {
    // Taken from Wireshark
    let mut bytes: Vec<u8> = vec![
        0x00, 0x01, 0x6d, 0x65, 0x6d, 0x74, 0x65, 0x73,
        0x74, 0x5f, 0x78, 0x38, 0x36, 0x2e, 0x30, 0x00,
        0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
    ];
    assert_eq!(TFTP::rrq("memtest_x86.0", "octet"), bytes);
    assert_eq!(TFTP::wrq("memtest_x86.0", "octet"), {
        bytes[1] = 0x02;
        bytes
    });
}

#[test]
fn ack_test() {
    assert_eq!(TFTP::ack(2137), vec![0x00, 0x04, 0x08, 0x59]);
    assert_eq!(TFTP::ack(33), vec![0x00, 0x04, 0x00, (1<<5) + 1]);
    assert_eq!(TFTP::ack(43), vec![0x00, 0x04, 0x00, 0x2b]);
}

#[test]
fn data_test() {
    assert_eq!(TFTP::data(43, vec![1, 2, 3]),
               vec![0x00, 0x03, 0x00, 0x2b, 1, 2, 3]);
}

#[test]
fn error_test() {
    let bytes: Vec<u8> = vec![
        0x00, 0x05, 0x00, 0x00, 0x54, 0x46, 0x54, 0x50,
        0x20, 0x41, 0x62, 0x6f, 0x72, 0x74, 0x65, 0x64,
        0x00
    ];
    assert_eq!(TFTP::error(0, "TFTP Aborted"), bytes)
}
