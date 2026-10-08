use std::net::Ipv4Addr;

use serde::Serialize;
use utoipa::ToSchema;

/// An address of this computer that another device on its network can connect to.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NetworkAddress {
    /// The network connection it belongs to, such as `wlan0` or `Wi-Fi`.
    pub name: String,
    /// The IPv4 address.
    #[schema(value_type = String, format = Ipv4)]
    pub address: Ipv4Addr,
}

/// Where this server is, as far as printing to it goes.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NetworkInfo {
    /// The addresses of this computer on its networks, without its own `127.0.0.1`.
    pub addresses: Vec<NetworkAddress>,
}

impl NetworkInfo {
    pub fn here() -> Self {
        // An address on a network of one is not one that other devices can be on, such as the second
        // address of `lo` that WSL adds.
        // An address on a network of one is not one that other devices can be on, such as the second
        // address of `lo` that WSL adds.
        let mut addresses: Vec<NetworkAddress> = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .filter(|interface| interface.is_oper_up())
            .filter_map(|interface| match interface.addr {
                if_addrs::IfAddr::V4(v4)
                    if !v4.ip.is_loopback() && !v4.ip.is_link_local() && v4.prefixlen < 32 =>
                {
                    Some(NetworkAddress {
                        name: interface.name,
                        address: v4.ip,
                    })
                }
                _ => None,
            })
            .collect();
        addresses.sort_by(|a, b| (&a.name, a.address).cmp(&(&b.name, b.address)));
        Self { addresses }
    }
}
