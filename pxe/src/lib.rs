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

    pub fn start(self) -> Self {
        self.option(6, &[0b00001100])
    }

    pub fn end(self) -> Self {
        self.option(255, &[])
    }

    pub fn menu_prompt(self, text: &str) -> Self {
        let mut v = vec![ &[0][..], text.as_bytes() ].concat();
        self.option(10, &v[..])
    }

    pub fn boot_servers(self, ips: Vec<Ipv4Addr>) -> Self {
        let ips_bytes = ips
            .iter()
            .map(|ip| ip.octets())
            .collect::<Vec<[u8; 4]>>()
            .concat();

        let mut field = vec![0, 0, ips.len() as u8];
        field.extend(ips_bytes);

        self.option(8, &field[..])
    }

    pub fn mcast(self, addr: Ipv4Addr) -> Self {
        self.option(7, &addr.octets()[..])
    }

    pub fn build(self) -> Vec<u8> {
        self.options.into_iter()
            .fold(Vec::new(), |mut acc, x| {
                let mut v = x.data.to_vec();
                if x.code != 0xFF {
                    v.insert(0, x.len());
                }
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
