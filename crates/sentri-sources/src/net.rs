//! Network address policy used to prevent SSRF.
//!
//! SENTRI fetches URLs typed by the user and URLs found in feeds. Neither may
//! reach the local machine, the LAN, the Tailscale network or cloud metadata
//! endpoints, so only globally routable addresses are allowed.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Whether `ip` is a public, globally routable unicast address.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    let blocked = ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0                                   // "this network"
        || a >= 240                                 // reserved
        || (a == 100 && (64..=127).contains(&b))    // CGNAT, includes Tailscale
        || (a == 192 && b == 0 && c == 0)           // IETF protocol assignments
        || (a == 198 && (18..=19).contains(&b)); // benchmarking
    !blocked
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = embedded_v4(ip) {
        return is_public_v4(v4);
    }
    let first = ip.segments()[0];
    let blocked = ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00                      // unique local
        || (first & 0xffc0) == 0xfe80                      // link local
        || (first & 0xffc0) == 0xfec0                      // site local (deprecated)
        || (first == 0x2001 && ip.segments()[1] == 0x0db8) // documentation
        || first == 0x2002; // 6to4 may wrap private IPv4
    !blocked
}

/// IPv4 addresses hidden in IPv4-mapped, IPv4-compatible and NAT64 forms.
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return Some(v4);
    }
    let s = ip.segments();
    let o = ip.octets();
    let tail = || Ipv4Addr::new(o[12], o[13], o[14], o[15]);
    if s[..6] == [0; 6] && !ip.is_unspecified() && !ip.is_loopback() {
        return Some(tail()); // deprecated IPv4-compatible
    }
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2..6] == [0; 4] {
        return Some(tail()); // NAT64 well-known prefix
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public(s: &str) -> bool {
        is_public(s.parse().unwrap())
    }

    #[test]
    fn allows_public_addresses() {
        assert!(public("93.184.216.34"));
        assert!(public("1.1.1.1"));
        assert!(public("2606:4700:4700::1111"));
    }

    #[test]
    fn blocks_internal_addresses() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.100.100.100",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::127.0.0.1",
            "64:ff9b::10.0.0.1",
            "2002:7f00:1::",
        ] {
            assert!(!public(ip), "{ip} should be blocked");
        }
    }
}
