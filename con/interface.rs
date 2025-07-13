use std::{
   io,
   net,
   pin::Pin,
   task,
};

use cyn::ResultExt as _;
use tokio::io::{
   AsyncRead,
   AsyncWrite,
   ReadBuf,
};

pub struct Interface {
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

impl AsyncRead for Interface {
   fn poll_read(
      mut self: Pin<&mut Self>,
      context: &mut task::Context<'_>,
      buffer: &mut ReadBuf<'_>,
   ) -> task::Poll<io::Result<()>> {
      Pin::new(&mut self.device).poll_read(context, buffer)
   }
}

impl AsyncWrite for Interface {
   fn poll_write(
      mut self: Pin<&mut Self>,
      context: &mut task::Context<'_>,
      buffer: &[u8],
   ) -> task::Poll<Result<usize, io::Error>> {
      Pin::new(&mut self.device).poll_write(context, buffer)
   }

   fn poll_flush(
      mut self: Pin<&mut Self>,
      context: &mut task::Context<'_>,
   ) -> task::Poll<Result<(), io::Error>> {
      Pin::new(&mut self.device).poll_flush(context)
   }

   fn poll_shutdown(
      mut self: Pin<&mut Self>,
      context: &mut task::Context<'_>,
   ) -> task::Poll<Result<(), io::Error>> {
      Pin::new(&mut self.device).poll_shutdown(context)
   }
}
