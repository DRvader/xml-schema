mod rust_codegen;
mod xml_element;

pub use rust_codegen::{
  Block, Enum, Field, Fields, Formatter, Function, Impl, Item, Module, Struct, Type, TypeDef,
  Variant,
};
pub use xml_element::XMLElement;
use xsd_types::XsdParseError;

pub enum GenType {
  Attribute,
  Content,
}

pub struct GenState {
  is_root: bool,
  gen_state: GenType,
}

pub trait XsdGen
where
  Self: Sized,
{
  fn gen(element: XMLElement, gen_state: GenState) -> Result<Self, XsdParseError>;
}

pub trait FromXmlString
where
  Self: Sized,
{
  fn from_xml(string: &str) -> Result<Self, String>;
}

impl FromXmlString for String {
  fn from_xml(string: &str) -> Result<Self, String> {
    Ok(string.to_string())
  }
}

macro_rules! gen_simple_parse_from_xml_string {
  ($ty: ty) => {
    impl FromXmlString for $ty {
      fn from_xml(string: &str) -> Result<Self, String> {
        string.parse::<$ty>().map_err(|e| e.to_string())
      }
    }
  };
}

gen_simple_parse_from_xml_string!(isize);
gen_simple_parse_from_xml_string!(usize);
gen_simple_parse_from_xml_string!(i64);
gen_simple_parse_from_xml_string!(u64);
gen_simple_parse_from_xml_string!(i32);
gen_simple_parse_from_xml_string!(u32);
gen_simple_parse_from_xml_string!(i8);
gen_simple_parse_from_xml_string!(u8);
