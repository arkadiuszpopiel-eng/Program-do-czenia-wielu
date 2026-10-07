//! Klasyfikacja adresów IP: publiczny = osiągalny w Internecie i nie lokalny. Wszystko, czego nie
//! da się jednoznacznie uznać za publiczne, jest traktowane jako niepubliczne (fail-closed).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Odmowa po rozwiązaniu nazwy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    /// Host nie ma żadnego adresu.
    #[error("host {0} nie ma adresu")]
    NoAddress(String),
    /// Host wskazuje adres niepubliczny (sieć lokalna, pętla — możliwy DNS rebinding).
    #[error(
        "host {host} wskazuje adres niepubliczny {addr} — odmowa (sieć lokalna, DNS rebinding)"
    )]
    NonPublic {
        /// Host.
        host: String,
        /// Pierwszy adres niepubliczny.
        addr: IpAddr,
    },
}

fn is_public_v4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    !(v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_multicast()
        || v4.is_documentation()
        || o[0] == 0
        || (o[0] == 100 && (64..128).contains(&o[1]))
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 192 && o[1] == 88 && o[2] == 99)
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
        || o[0] >= 240)
}

fn embedded_v4(hi: u16, lo: u16) -> Ipv4Addr {
    Ipv4Addr::from((u32::from(hi) << 16) | u32::from(lo))
}

fn is_public_v6(v6: Ipv6Addr) -> bool {
    if let Some(v4) = v6.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    let s = v6.segments();
    // NAT64 (64:ff9b::/96) niesie adres IPv4 — o publiczności decyduje on.
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return is_public_v4(embedded_v4(s[6], s[7]));
    }
    // 6to4 (2002::/16) — adres IPv4 w segmentach 1–2.
    if s[0] == 0x2002 {
        return is_public_v4(embedded_v4(s[1], s[2]));
    }
    let global_unicast = (s[0] & 0xe000) == 0x2000;
    // 2001::/23 (Teredo, ORCHID, benchmark…), 2001:db8::/32 i 3fff::/20 (dokumentacja).
    let special = match s[0] {
        0x2001 => s[1] < 0x0200 || s[1] == 0x0db8,
        0x3fff => s[1] < 0x1000,
        _ => false,
    };
    global_unicast && !special
}

/// Czy adres IP jest publiczny (nie: pętla, prywatne, link-local, CGNAT, multicast, dokumentacja,
/// zarezerwowane, nieokreślony; IPv6: tylko `2000::/3` bez Teredo/ORCHID/dokumentacji, mapowane
/// IPv4, NAT64 i 6to4 według osadzonego adresu IPv4).
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

/// Wszystkie adresy z DNS muszą być publiczne — jeden niepubliczny (np. rebinding z wieloma
/// rekordami A) odrzuca cały host.
pub fn check_resolved(host: &str, addrs: &[IpAddr]) -> Result<(), GuardError> {
    if addrs.is_empty() {
        return Err(GuardError::NoAddress(host.to_owned()));
    }
    match addrs.iter().find(|a| !is_public_ip(**a)) {
        Some(addr) => Err(GuardError::NonPublic {
            host: host.to_owned(),
            addr: *addr,
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn public_and_non_public_addresses() {
        for p in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "2606:4700::1111",
            "2a00:1450:4001::200e",
            "64:ff9b::808:808",
            "2002:808:808::1",
            "::ffff:8.8.8.8",
        ] {
            assert!(is_public_ip(ip(p)), "{p}");
        }
        for n in [
            "127.0.0.1",
            "127.255.255.254",
            "10.0.0.1",
            "172.16.5.4",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "100.127.255.255",
            "0.0.0.0",
            "0.1.2.3",
            "192.0.0.8",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.7",
            "203.0.113.9",
            "224.0.0.251",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::127.0.0.1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::7f00:1",
            "64:ff9b:1::1",
            "2002:7f00:1::1",
            "2002:c0a8:101::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "2001:db8::1",
            "2001::1",
            "2001:10::1",
            "3fff::1",
            "100::1",
            "1::1",
            "4000::1",
        ] {
            assert!(!is_public_ip(ip(n)), "{n}");
        }
    }

    #[test]
    fn resolution_is_all_or_nothing() {
        assert!(check_resolved("ok.pl", &[ip("8.8.8.8"), ip("1.1.1.1")]).is_ok());
        let e = check_resolved("rebind.pl", &[ip("8.8.8.8"), ip("127.0.0.1")]).unwrap_err();
        assert!(matches!(e, GuardError::NonPublic { addr, .. } if addr == ip("127.0.0.1")));
        assert!(e.to_string().contains("rebinding"));
        assert_eq!(
            check_resolved("pusty.pl", &[]),
            Err(GuardError::NoAddress("pusty.pl".into()))
        );
    }
}
