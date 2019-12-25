#![allow(safe_packed_borrows, unused_must_use)]

#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate arrayref;

use std::mem;
use std::default::Default;
use std::fmt::{Display, Formatter, Result};
use std::net::Ipv4Addr;
use std::borrow::Borrow;

use phf::{Map, phf_map};

#[repr(packed)]
#[derive(Builder, Copy)]
#[builder(default)]
pub struct DHCPBody {
    pub op: u8,
    pub htype: u8,
    pub hlen: u8,
    pub hops: u8,
    pub xid: u32,
    pub secs: u16,
    pub flags: u16,
    pub ciaddr: [u8; 4],
    pub yiaddr: [u8; 4],
    pub siaddr: [u8; 4],
    pub giaddr: [u8; 4],
    pub chaddr: [u8; 16],
    pub sname: [u8; 64],
    pub filename: [u8; 128],
    pub mcookie: u32
}

#[derive(Clone)]
pub struct DHCPOption(pub u8, pub u8, pub Vec<u8>);

#[derive(Clone)]
pub struct DHCPDgram {
    pub body: DHCPBody,
    pub options: Vec<DHCPOption>
}

impl DHCPDgram {
    pub fn swap_endianess(&self) -> Self {
        Self {
            body: self.body.swap_endianess(),
            ..self.clone()
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // Not enough bytes.
        if bytes.len() < mem::size_of::<DHCPBody>() {
            return None;
        }

        let main = &bytes[..mem::size_of::<DHCPBody>()];
        let rest = &bytes[mem::size_of::<DHCPBody>()..];

        let mut dhcp_buff = [0u8; mem::size_of::<DHCPBody>()];
        dhcp_buff.copy_from_slice(&main[..]);

        let dhcp: DHCPBody = unsafe { mem::transmute(dhcp_buff) };
        let options: Vec<DHCPOption> = read_options(rest);

        Some(Self {
            body: dhcp.clone(),
            options: options
        })
    }

    pub fn as_bytes(self) -> Vec<u8> {
        let mut body_buff = [0u8; mem::size_of::<DHCPBody>()];
        body_buff = unsafe { mem::transmute(self.body) };

        let options_buff = self.options.into_iter()
            .fold(Vec::<u8>::new(), |mut acc, x| {
                let mut v = x.2.to_vec();
                v.insert(0, x.1);
                v.insert(0, x.0);
                acc.extend(&v);
                acc
            });

        vec![body_buff.to_vec(), options_buff].concat()
    }

    pub fn option(&self, id: u8) -> Option<&[u8]> {
        self.options.iter()
            .filter(|option| option.0 == id)
            .map(|option| &option.2[..])
            .next()
    }
}

#[derive(Default)]
pub struct DHCPDgramBuilder {
    dhcp: Option<DHCPBody>,
    options: Vec<DHCPOption>
}

impl DHCPDgramBuilder {
    pub fn option(mut self, code: u8, data: &[u8]) -> Self {
        self.options.push(DHCPOption(code, data.len() as u8, data.to_vec()));
        self
    }

    pub fn body(mut self, dhcp: DHCPBody) -> Self {
        self.dhcp = Some(dhcp);
        self
    }

    pub fn end(self) -> Self {
        self.option(0xFF, &[])
    }

    pub fn build(self) -> Option<DHCPDgram> {
        let options = self.options;
        self.dhcp.map(|body| {
            DHCPDgram {
                body: body,
                options: options
            }
        })
    }
}

impl DHCPBody {
    fn swap_endianess(&self) -> Self {
        Self {
            xid: self.xid.swap_bytes(),
            secs: self.secs.swap_bytes(),
            flags: self.flags.swap_bytes(),
            mcookie: self.mcookie.swap_bytes(),
            ..self.clone()
        }
    }
}

// Parse byte array with options.
fn read_options(data: &[u8]) -> Vec<DHCPOption> {
    let mut idx = 0;
    let mut options = Vec::with_capacity(256);

    loop {
        let option = match (data.get(idx), data.get(idx+1)) {
            // Padding byte
            (Some(0x00), _) => {
                idx += 1;
                Some(DHCPOption(0x00, 0, vec![]))
            },
            // Option byte
            (Some(code), Some(length)) => {
                let length_us = *length as usize;
                if idx+length_us+2 > data.len() {
                    None
                } else {
                    let option = DHCPOption(*code, *length,
                                            data[idx+2..idx+length_us+2].to_vec());
                    idx += length_us+2;
                    Some(option)
                }
            },
            // Propbably end of the data
            _ => None
        };

        if option.is_none() {
            break;
        }
        options.push(option.unwrap());
    }

    options
}

impl Default for DHCPDgram {
    fn default() -> Self {
        Self {
            options: Vec::new(),
            body: Default::default()
        }
    }
}

impl Default for DHCPBody {
    fn default() -> Self {
        let bytes = [0u8; mem::size_of::<DHCPBody>()];
        let mut body: DHCPBody = unsafe {
            mem::transmute(bytes)
        };
        body.mcookie = 0x63825363;
        body
    }
}

impl Clone for DHCPBody { fn clone(&self) -> Self { *self } }

static DHCP_OPERATION: Map<u8, &'static str> = phf_map! {
    0x01u8 => "BOOT REQUEST",
    0x02u8 => "BOOT REPLY"
};

static DHCP_MESSAGE_TYPE: Map<u8, &'static str> = phf_map! {
    1u8 => "DISCOVER",
    2u8 => "OFFER",
    3u8 => "REQUEST",
    4u8 => "DECLINE",
    5u8 => "ACK",
    6u8 => "NACK",
    7u8 => "RELEASE",
    8u8 => "INFORM"
};

static DHCP_OPTION_NAME: Map<u8, &'static str> = phf_map! {
    1u8 => "Subnet Mask",
    3u8 => "Router",
    6u8 => "Domain Name Server",

    15u8 => "Domain Name",

    43u8 => "Vendor-Specific Information (PXEClient)",

    51u8 => "IP Address Lease Time",
    52u8 => "Overload 'sname' or 'file'",
    53u8 => "DHCP Message Type",
    54u8 => "DHCP Server Identifier",
    55u8 => "Parameter Request List",
    57u8 => "Maximum DHCP Message Size",
    58u8 => "Renewal Time Value",
    59u8 => "Rebinding Time Value",
    60u8 => "Vendor class Identifier",
    61u8 => "Client Identifier",

    93u8 => "Client System Architecture",
    94u8 => "Client Network Device Interface",
    97u8 => "UUID/GUID-based Client Identifier",

    255u8 => "END"
};

impl Display for DHCPDgram {
    fn fmt(&self, f: &mut Formatter) -> Result {
        writeln!(f, "{}", self.body);
        writeln!(f, "OPTIONS:");

        for option in &self.options {
            if option.0 == 0x00 {
                break;
            }
            writeln!(f, "---");
            write!(f, "{}", option);
        }

        Ok(())
    }
}

impl Display for DHCPBody {
    fn fmt(&self, f: &mut Formatter) -> Result {
        writeln!(f, "TYPE: {}", DHCP_OPERATION.get(&self.op).unwrap_or(&"NONE"));
        writeln!(f, "Network type: 0x{:02x}", self.htype);
        writeln!(f, "XID: 0x{:x}", self.xid);
        writeln!(f, "Client: {} | Your: {}", ipv4_str(self.ciaddr), ipv4_str(self.yiaddr));
        writeln!(f, "Server: {} | Gateway: {}", ipv4_str(self.siaddr), ipv4_str(self.giaddr));
        writeln!(f, "Client MAC: {}", mac_str(array_ref![self.chaddr, 0, 6]));
        writeln!(f, "Server Name: {}", std::str::from_utf8(&self.sname[..]).unwrap_or(""));
        writeln!(f, "Bootfile: {}", std::str::from_utf8(&self.filename[..]).unwrap_or(""));
        write!(f, "COOKIE: 0x{:08x}", self.mcookie)
    }
}

impl Display for DHCPOption {
    fn fmt(&self, f: &mut Formatter) -> Result {
        let name = DHCP_OPTION_NAME
            .get(&self.0)
            .unwrap_or(&"Unknown");

        writeln!(f, "OPTION {} - '{}', LENGTH: {}", self.0, name, self.1);
        match self.0 {
            53 => {
                let msg_type_name = DHCP_MESSAGE_TYPE
                    .get(&self.2[0])
                    .unwrap_or(&"Unknown");
                writeln!(f, "{}", msg_type_name)
            },
            _ => writeln!(f, "DATA: {:?}", self.2)
        }
    }
}

fn ipv4_str(octets: impl Borrow<[u8; 4]>) -> String {
    let octets = octets.borrow();
    Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]).to_string()
}

fn mac_str(octets: impl Borrow<[u8; 6]>) -> String {
    let octets = octets.borrow();
    format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            octets[0], octets[1], octets[2],
            octets[3], octets[4], octets[5])
}
