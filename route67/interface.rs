use std::{
   io,
   net,
};

use super::address;

pub const MTU: u16 = 1420;

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to create tun device")]
   CreateDevice(#[source] io::Error),

   #[cfg(target_os = "linux")]
   #[error("failed to create netlink connection")]
   CreateNetlinkConnection(#[source] io::Error),

   #[cfg(target_os = "linux")]
   #[error("failed to get tun interface index")]
   GetInterfaceIndex(#[source] io::Error),

   #[cfg(target_os = "linux")]
   #[error("failed to add local route for host prefix")]
   AddLocalRoute(#[source] rtnetlink::Error),
}

pub struct Interface {
   device: tun_rs::AsyncDevice,
}

impl Interface {
   #[cfg_attr(
      not(target_os = "linux"),
      expect(
         clippy::unused_async,
         reason = "await used on linux for netlink route addition"
      )
   )]
   pub async fn create(name: Option<&str>, prefix: address::Prefix) -> Result<Self, Error> {
      let mut builder = tun_rs::DeviceBuilder::new()
         .ipv6(
            net::Ipv6Addr::from({
               let mut addr = [0_u8; size_of::<net::Ipv6Addr>()];
               addr[..prefix.len()].copy_from_slice(&*prefix);
               // ::1 in the subnet portion.
               *addr.last_mut().expect("address array must not be empty") = 1;
               addr
            }),
            u8::try_from(address::VPN_PREFIX.len() * 8).expect("prefix must fit in u8"),
         )
         .mtu(MTU);

      if let Some(name) = name {
         builder = builder.name(name);
      }

      let device = builder.build_async().map_err(Error::CreateDevice)?;

      // Add an RTN_LOCAL route for the host's own /80 prefix so the kernel delivers
      // all traffic for it locally.
      #[cfg(target_os = "linux")]
      {
         const RT_TABLE_LOCAL: u32 = 255;

         use rtnetlink::packet_route::route;

         let (connection, handle, _) =
            rtnetlink::new_connection().map_err(Error::CreateNetlinkConnection)?;
         tokio::spawn(connection);

         handle
            .route()
            .add(
               rtnetlink::RouteMessageBuilder::<net::Ipv6Addr>::new()
                  .destination_prefix(
                     net::Ipv6Addr::from(prefix),
                     u8::try_from(address::HOST_PREFIX_RANGE.end * 8)
                        .expect("prefix length must fit in u8"),
                  )
                  .output_interface(device.if_index().map_err(Error::GetInterfaceIndex)?)
                  .kind(route::RouteType::Local)
                  .scope(route::RouteScope::Host)
                  .table_id(RT_TABLE_LOCAL)
                  .build(),
            )
            .execute()
            .await
            .map_err(Error::AddLocalRoute)?;

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
