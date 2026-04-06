use std::{
   io,
   net,
};

use cyn::ResultExt as _;

use super::address;

pub const MTU: u16 = 1420;

pub struct Interface {
   device: tun_rs::AsyncDevice,
}

impl Interface {
   #[expect(
      clippy::unused_async,
      reason = "await used on linux for netlink route addition"
   )]
   pub async fn create(name: Option<&str>, prefix: address::Prefix) -> cyn::Result<Self> {
      let mut builder = tun_rs::DeviceBuilder::new()
         .ipv6(
            net::Ipv6Addr::from({
               let mut addr = [0_u8; size_of::<net::Ipv6Addr>()];
               addr[..prefix.len()].copy_from_slice(&*prefix);
               // ::1 in the subnet portion.
               *addr.last_mut().expect("non-empty") = 1;
               addr
            }),
            u8::try_from(address::VPN_PREFIX.len() * 8).expect("prefix fits in u8"),
         )
         .mtu(MTU);

      if let Some(name) = name {
         builder = builder.name(name);
      }

      let device = builder
         .build_async()
         .chain_err("failed to create tun device")?;

      // Add an RTN_LOCAL route for the host's /80 prefix so the kernel delivers
      // all traffic for it to the local stack.
      #[cfg(target_os = "linux")]
      {
         const RT_TABLE_LOCAL: u32 = 255;

         let (connection, handle, _) =
            rtnetlink::new_connection().chain_err("failed to create netlink connection")?;
         tokio::spawn(connection);

         handle
            .route()
            .add(
               rtnetlink::RouteMessageBuilder::<net::Ipv6Addr>::new()
                  .destination_prefix(
                     net::Ipv6Addr::from(prefix),
                     u8::try_from(address::HOST_PREFIX_RANGE.end * 8)
                        .expect("prefix length fits in u8"),
                  )
                  .output_interface(
                     device
                        .if_index()
                        .chain_err("failed to get tun interface index")?,
                  )
                  .kind(rtnetlink::packet_route::route::RouteType::Local)
                  .scope(rtnetlink::packet_route::route::RouteScope::Host)
                  .table_id(RT_TABLE_LOCAL)
                  .build(),
            )
            .execute()
            .await
            .chain_err("failed to add local route for host prefix")?;

         tracing::info!(prefix = %net::Ipv6Addr::from(prefix), "Added local route for host prefix");
      }

      Ok(Self { device })
   }

   pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
      self.device.recv(buf).await
   }

   pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
      self.device.send(buf).await
   }
}
