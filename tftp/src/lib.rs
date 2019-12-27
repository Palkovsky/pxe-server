use std::io;
use std::io::Read;

use std::fs;
use std::fs::File;
use std::path::{
    PathBuf
};

use std::num::Wrapping;

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

/*
 * TODO:
 * - Security features.
 * - Better logging.
 * - Mult-threading
 * - Better error handling.
 */

const ROOT_DIR: &'static str = "R:\\tftpboot";

impl TFTPTransfer {
    fn new(block_sz: u16, path_str: impl Into<String>) -> io::Result<Self> {

        let path_str = path_str.into();
        let path = PathBuf::from(&path_str);

        let mut base = PathBuf::from(ROOT_DIR);
        println!("PATH: {:?}", path);
        base.push(PathBuf::from(path));

        println!("FILPATH: {:?}", base);

        let fil = File::open(base)?;
        Ok(TFTPTransfer {
            block_cnt: 0,
            done: false,
            block_sz: block_sz,
            file: fil
        })
    }

    fn next_block(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }

        let mut buff = vec![0; self.block_sz as usize];
        let bytes_read = self.file.read(&mut buff)
            .unwrap_or(0);

        self.block_cnt = (Wrapping(self.block_cnt) + Wrapping(1)).0;
        self.done = bytes_read as u16 != self.block_sz;

        // Ciebie trzeba skrócić troszeczkę
        let buff_trim = &buff[..bytes_read];
        Some(buff_trim.to_vec())
    }

    fn tsize(&self) -> usize {
        self.file.metadata()
            .map(|meta| meta.len() as usize)
            .unwrap_or(0)
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


    fn listen(&mut self, socket: &UdpSocket) -> io::Result<()> {
        let mut buf = [0; 4096];
        let (amt, from) = socket.recv_from(&mut buf)?;

        let buf = &mut buf[..amt];
        let res = match buf.get(1) {
            // Read Request
            Some(&TFTP::RRQ) => {
                // Parse request here
                let res = TFTP::parse_rrq(&buf)
                    .map_err(|err| {println!("{}", err); err})
                    .map(|transfer| {
                        let ack = TFTP::opt_ack(Some(transfer.block_sz), Some(transfer.tsize()));
                        self.transfers.insert(from.clone(), transfer);
                        ack
                    })
                    .unwrap_or(TFTP::error(1, "No such file."));
                Ok(res)
            },

            // Transfers next or returns error if transfer impossible.
            Some(&TFTP::ACK) =>
                self.send_next(&from),

            // Send error if something other than ACK or RRQ.
            Some(_) =>
                Ok(TFTP::error(20, "Unsuported operation.")),

            _ =>
                Err(io::Error::new(io::ErrorKind::InvalidData, "Not enough data."))
        }?;

        socket.send_to(res.as_slice(), &from)?;
        Ok(())
    }

    fn send_next(&mut self, to: &SocketAddr) -> io::Result<Vec<u8>> {
        let is_done = self.transfers.get_mut(&to).map(|t| t.done).unwrap_or(false);
        if is_done {
            self.transfers.remove(to);
            return Err(io::Error::new(io::ErrorKind::ConnectionRefused, "Transfer finished."));
        }

        Ok(
            self.transfers.get_mut(&to)
                .and_then(|transfer| transfer.next_block().map(|blk| (transfer, blk)))
                .map(|(transfer, bytes)| TFTP::data(transfer.block_cnt, bytes))
                .unwrap_or(TFTP::error(2, "Unable to read next block."))
        )
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
        vec![vec![Self::WRQ, 0x02],
             str_to_bytes(filname.into()),
             str_to_bytes(mode.into())
        ].concat()
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

    pub fn opt_ack(blksize: Option<u16>, tsize: Option<usize>) -> Vec<u8> {
        let option_to_bytes = |name: &str, option: Option<String>| {
            option
                .map(|num| vec![str_to_bytes(name), str_to_bytes(num)].concat())
                .unwrap_or(Vec::new())
        };

        let blksize = option_to_bytes("blksize", blksize.map(|sz| sz.to_string()));
        let tsize = option_to_bytes("tsize", tsize.map(|sz| sz.to_string()));

        vec![vec![0x00, Self::OPT_ACK], blksize, tsize].concat()
    }

    pub fn error(code: u16, msg: impl Into<String>) -> Vec<u8> {
        let lo = (code & 0xFF) as u8;
        let hi = (code >> 8) as u8;

        vec![vec![0x00, Self::ERROR, hi, lo], str_to_bytes(msg.into())].concat()
    }

    pub fn parse_rrq(bytes: &[u8]) -> io::Result<TFTPTransfer> {
        let opcode = bytes.get(0)
            .and_then(|b1| bytes.get(1).map(|b2| (b1.clone(), b2.clone())))
            .unwrap_or((0, 0));

        if opcode != (0, Self::RRQ) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid opcode."));
        }

        let rest = &bytes[2..].to_vec();
        let chunks = rest
            .split(|b| *b == 0x00)
            .collect::<Vec<&[u8]>>();
        let mut windows = chunks
            .windows(2);

        if let Some(&[filname, _mode]) = windows.next() {
            let filname_str = std::str::from_utf8(filname)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to parse filename string."))?;
            let _mode_str = std::str::from_utf8(filname)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to parse mode string."))?;

            let options = windows.fold(HashMap::<String, String>::new(), |mut acc, window| {
                let key   = window.get(0).and_then(|option_name| std::str::from_utf8(option_name).ok());
                let value = window.get(1).and_then(|option_value| std::str::from_utf8(option_value).ok());

                if let (Some(key), Some(value)) = (key, value) {
                    acc.insert(key.to_string(), value.to_string());
                }

                acc
            });

            let blksize = options.get("blksize")
                .and_then(|string| string.parse::<u16>().ok())
                .unwrap_or(1488);

            return TFTPTransfer::new(blksize, filname_str);
        }

        return Err(io::Error::new(io::ErrorKind::InvalidData, "Missing filename or mode."));
    }
}

fn str_to_bytes(string: impl Into<String>) -> Vec<u8> {
    vec![string.into().as_bytes(), &[0x00][..]].concat()
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
