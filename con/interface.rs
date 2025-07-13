use std::net;

use cyn::ResultExt as _;
use derive_more::{
   Deref,
   DerefMut,
};

#[derive(Deref, DerefMut)]
pub struct Interface {
   #[deref]
   #[deref_mut]
   device: tun::AsyncDevice,

   pub v4: net::Ipv4Addr,
   pub v6: net::Ipv6Addr,
}

impl Interface {
   pub fn create(name: &str, v4: net::Ipv4Addr, v6: net::Ipv6Addr) -> cyn::Result<Self> {
      let mut config = tun::Configuration::default();

      config.tun_name(name);
      config.address(v4);
      config.netmask(net::Ipv4Addr::new(255, 255, 0, 0));
      config.mtu(1420);
      config.up();

      #[cfg(any(target_os = "macos", target_os = "ios"))]
      config.platform_config(|config| {
         config.packet_information(false);
      });

      let device = tun::create_as_async(&config).chain_err("failed to create tun device")?;

      tracing::info!("Created TUN interface '{name}' with IPv4 {v4} and IPv6 {v6}.");

      Ok(Self { device, v4, v6 })
   }
}
