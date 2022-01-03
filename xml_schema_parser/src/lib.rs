#[macro_use]
extern crate yaserde_derive;

#[macro_use]
extern crate quote;

mod codegen;
mod xsd;

pub use xsd::Xsd;
