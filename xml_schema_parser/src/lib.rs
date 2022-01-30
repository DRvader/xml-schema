#[macro_use]
extern crate quote;

mod codegen;
mod xsd;

pub use xsd::XMLElementWrapper;
pub use xsd::{Xsd, XsdError};

pub trait XsdParse
where
  Self: Sized,
{
  fn parse(element: XMLElementWrapper) -> Result<Self, XsdError>;
}
