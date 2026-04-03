use std::{
   io,
   iter,
   net,
};

use cyn::ResultExt as _;

use super::address;

pub const MTU: u16 = 1420;

pub struct Interface {
   device: tun_rs::AsyncDevice,
}

impl Interface {
   pub fn create(name: Option<&str>, prefix: address::Prefix) -> cyn::Result<Self> {
      let mut builder = tun_rs::DeviceBuilder::new()
         .ipv6(
            net::Ipv6Addr::from(
               <[u8; _]>::try_from(
                  prefix
                     .into_iter()
                     .chain(iter::repeat(0))
                     .take(size_of::<net::Ipv6Addr>())
                     .collect::<Vec<_>>(),
               )
               .expect("size matches"),
            ),
            u8::try_from(address::Prefix::BITS).expect("prefix fits in u8"),
         )
         .mtu(MTU);

      if let Some(name) = name {
         builder = builder.name(name);
      }

      let device = builder
         .build_async()
         .chain_err("failed to create tun device")?;

      Ok(Self { device })
   }

   pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
      self.device.recv(buf).await
   }

   pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
      self.device.send(buf).await
   }
}
