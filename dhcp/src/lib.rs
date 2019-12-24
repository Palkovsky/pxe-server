#![allow(safe_packed_borrows, unused_must_use)]

#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate arrayref;


#[repr(packed)]
#[derive(Builder, Copy)]
#[builder(default)]
pub struct DHCPDgram {
    op: u8,
    htype: u8,
    hlen: u8,
    hops: u8,
    xid: u32,
    secs: u16,
    flags: u16,
    ciaddr: [u8; 4],
    yiaddr: [u8; 4],
    siaddr: [u8; 4],
    giaddr: [u8; 4],
    chaddr: [u8; 16],
    sname: [u8; 64],
    filename: [u8; 128],
    mcookie: u32
}

pub struct DHCPOptionsBuilder {
    options: HashMap<u8, Vec<u8>>
}

static MESSAGE_TYPE: Map<u8, &'static str> = phf_map! {
    0x01u8 => "BOOT REQUEST",
    0x02u8 => "BOOT REPLY"
};

use std::mem;
use std::default::Default;
use std::fmt::{Display, Formatter, Result};
use std::net::Ipv4Addr;
use std::collections::HashMap;
use std::borrow::Borrow;

use phf::{ Map, phf_map };

impl DHCPDgram {
    pub fn flip_endianess(&self) -> Self {
        Self{
            xid: self.xid.swap_bytes(),
            secs: self.secs.swap_bytes(),
            flags: self.flags.swap_bytes(),
            mcookie: self.mcookie.swap_bytes(),
            ..self.clone()
        }
   }
}

impl DHCPOptionsBuilder {
    pub fn option(self, identifier: impl Into<String>, values: Vec<u8>) -> Self {
        let identifier: String = identifier.into();
        self
    }

    pub fn put(self, identifier: u8, values: Vec<u8>) -> Self {
        self
    }
}

impl Default for DHCPDgram {
    fn default() -> Self {
        let data = [0u8; mem::size_of::<DHCPDgram>()];
        let mut transmuted: DHCPDgram = unsafe { mem::transmute(data) };
        transmuted.mcookie = 0x63825363;
        transmuted
    }
}

impl Clone for DHCPDgram { fn clone(&self) -> Self { *self } }

impl Display for DHCPDgram {
    fn fmt(&self, f: &mut Formatter) -> Result {
        writeln!(f, "TYPE: {}", MESSAGE_TYPE.get(&self.op).unwrap_or(&"NONE"));
        writeln!(f, "Network type: 0x{:02x}", self.htype);
        writeln!(f, "XID: 0x{:x}", self.xid);
        writeln!(f, "Client: {} | Your: {}", ipv4_str(self.ciaddr), ipv4_str(self.yiaddr));
        writeln!(f, "Server: {} | Gateway: {}", ipv4_str(self.siaddr), ipv4_str(self.giaddr));
        writeln!(f, "Client MAC: {}", mac_str(array_ref![self.chaddr, 0, 6]));
        writeln!(f, "COOKIE: 0x{:08x}", self.mcookie)
    }
}

fn ipv4_str(octets: impl Borrow<[u8; 4]>) -> String {
    let octets = octets.borrow();
    Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]).to_string()
}

fn mac_str(octets: impl Borrow<[u8; 6]>) -> String {
    let octets = octets.borrow();
    format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            octets[0],
            octets[1],
            octets[2],
            octets[3],
            octets[4],
            octets[5])
}
