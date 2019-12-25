use std::net::Ipv4Addr;

#[derive(Default)]
pub struct PXEBuilder {
    options: Vec<PXEOption>
}

impl PXEBuilder {

    pub fn option(mut self, tag: u8, data: &[u8]) -> Self {
        let option = PXEOption { code: tag, data: data.to_vec() };
        self.options.push(option);
        self
    }

    pub fn start(self, discover: bool) -> Self {
        /*
        PXE_DISCOVERY_CONTROL
          bit 0 = If set, disable broadcast discovery.
          bit 1 = If set, disable multicast discovery.
          bit 2 = If set, only use/accept servers in PXE_BOOT_SERVERS.
          bit 3 = If set, and a boot file name is present in the
                  initial DHCP or ProxyDHCP offer packet, download
                  the boot file (do not prompt/menu/discover).
          bit 4-7 = Must be 0.
         */
        let mut byte = 0b00000110;
        byte |= (!discover as u8) << 3;
        self.option(6, &[byte])
    }

    pub fn end(self) -> Self {
        self.option(255, &[])
    }

    pub fn menu_prompt(self, timeout: u8, text: &str) -> Self {
        let v = vec![&[timeout][..], text.as_bytes()].concat();
        self.option(10, v.as_slice())
    }

    pub fn menu_items(self, items: Vec<&'static str>) -> Self {
        let bytes = items
            .iter()
            .map(|item| {
                // 2 bytes of server type, 1 byte of description length
                let mut pre = vec![8, 0, item.len() as u8];
                // description of specified size
                pre.extend(item.as_bytes());
                pre
            })
            .collect::<Vec<Vec<u8>>>()
            .concat();
        self.option(9, bytes.as_slice())
    }

    // Specify IPs of boot servers
    pub fn boot_servers(self, ips: Vec<&Ipv4Addr>) -> Self {
        let ips_bytes = ips
            .iter()
            .map(|ip| ip.octets())
            .collect::<Vec<[u8; 4]>>()
            .concat();

        let mut field = vec![0, 0, ips.len() as u8];
        field.extend(ips_bytes);

        self.option(8, field.as_slice())
    }

    pub fn mcast(self, addr: Ipv4Addr) -> Self {
        self.option(7, &addr.octets()[..])
    }

    pub fn build(self) -> Vec<u8> {
        self.options
            .into_iter()
            .fold(Vec::new(), |mut acc, x| {
                let mut v = x.data.to_vec();

                // Don't insert length of END option.
                if x.code != 0xFF {
                    v.insert(0, x.len());
                }
                // Insert code at the start.
                v.insert(0, x.code);

                acc.extend(&v);
                acc
            })
    }
}

#[derive(Default)]
struct PXEOption {
    code: u8,
    data: Vec<u8>
}

impl PXEOption {
    pub fn len(&self) -> u8 {
        self.data.len() as u8
    }
}
