// src/ip_pool.rs
use std::net::Ipv4Addr;

pub struct IpPool {
    available_ips: Vec<Ipv4Addr>,
    leased_ips: Vec<Ipv4Addr>,
}

impl IpPool {
    pub fn new(start: u8, end: u8) -> Self {
        let base = [192, 168, 1, 0];
        let mut available_ips = Vec::new();
        for i in start..=end {
            available_ips.push(Ipv4Addr::new(base[0], base[1], base[2], i));
        }
        IpPool {
            available_ips,
            leased_ips: Vec::new(),
        }
    }

    pub fn lease_ip(&mut self) -> Option<Ipv4Addr> {
        if let Some(ip) = self.available_ips.pop() {
            self.leased_ips.push(ip);
            Some(ip)
        } else {
            None
        }
    }

    pub fn confirm_lease(&mut self, ip: Ipv4Addr) -> bool {
        if self.leased_ips.contains(&ip) {
            true
        } else if let Some(pos) = self.available_ips.iter().position(|&x| x == ip) {
            self.available_ips.remove(pos);
            self.leased_ips.push(ip);
            true
        } else {
            false
        }
    }

    pub fn release_ip(&mut self, ip: Ipv4Addr) {
        if let Some(pos) = self.leased_ips.iter().position(|&x| x == ip) {
            self.leased_ips.remove(pos);
            self.available_ips.push(ip);
        }
    }
}
