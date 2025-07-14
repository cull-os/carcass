#![feature(trait_alias)]

pub mod address;

mod behaviour;
pub use behaviour::{
   Behaviour,
   BehaviourEvent,
   run,
};

pub mod config;
pub use config::Config;

mod interface;
pub use interface::{
   Interface,
   MTU,
};

pub mod ip;
