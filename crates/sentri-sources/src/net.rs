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
        || (a == 192 && b == 88 && c == 99)         // 6to4 relay anycast
        || (a == 198 && (18..=19).contains(&b)); // benchmarking
    !blocked
}

/// IPv6 uses an allowlist: only global unicast (`2000::/3`) minus
/// special-purpose ranges. Everything else (NAT64, ULA, link-local, IPv4
/// embeddings other than mapped addresses, discard-only…) is refused.
fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    let s = ip.segments();
    let global_unicast = (s[0] & 0xe000) == 0x2000;
    let special = match s[0] {
        0x2001 => s[1] < 0x0200 || s[1] == 0x0db8, // IETF assignments (Teredo…), documentation
        0x2002 => true,                            // 6to4 may wrap private IPv4
        0x3fff => s[1] < 0x1000,                   // 3fff::/20 documentation
        _ => false,
    };
    global_unicast && !special
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
            "64:ff9b:1::a00:1",
            "2001::1",
            "100::1",
            "::ffff:0:7f00:1",
            "3fff::1",
            "192.88.99.1",
        ] {
            assert!(!public(ip), "{ip} should be blocked");
        }
    }
}
