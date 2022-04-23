mod codegen_helper;
mod rust_codegen;
mod xml_element;

pub use rust_codegen::{
  Block, Enum, Field, Fields, Formatter, Function, Impl, Item, Module, Struct, Type, TypeDef,
  Variant,
};
pub use xml_element::XMLElement;
use xsd_types::{XsdGenError, XsdIoError};

pub use codegen_helper::{fromxml_impl, xsdgen_impl};

#[derive(Clone, Copy)]
pub enum GenType {
  Attribute,
  Content,
}

#[derive(Clone)]
pub struct GenState {
  is_root: bool,
  state: GenType,
}

pub trait XsdGen
where
  Self: Sized,
{
  fn gen(
    element: &mut XMLElement,
    gen_state: GenState,
    name: Option<&str>,
  ) -> Result<Self, XsdIoError>;
}

impl<T: XsdGen> XsdGen for Vec<T> {
  fn gen(
    element: &mut XMLElement,
    gen_state: GenState,
    name: Option<&str>,
  ) -> Result<Self, XsdIoError> {
    let output = match gen_state.state {
      GenType::Attribute => {
        let mut new_state = gen_state.clone();
        new_state.is_root = false;
        vec![T::gen(element, new_state, name)?]
      }
      GenType::Content => {
        if let Some(name) = name {
          let mut new_state = gen_state.clone();
          new_state.is_root = false;
          element.get_children_with(name, |mut value| {
            T::gen(&mut value, new_state.clone(), Some(name))
          })?
        } else {
          return Err(XsdGenError {
            node_name: element.node_name(),
            ty: xsd_types::XsdType::Unknown,
            msg: format!("Expected node name to parse vector got None."),
          })?;
        }
      }
    };

    // if gen_state.is_root {
    //   element.finalize(false, false)?;
    // }

    Ok(output)
  }
}

impl<T: XsdGen> XsdGen for Option<T> {
  fn gen(
    element: &mut XMLElement,
    gen_state: GenState,
    name: Option<&str>,
  ) -> Result<Self, XsdIoError> {
    if let Some(name) = name {
      let output = match gen_state.state {
        GenType::Attribute => {
          let mut new_state = gen_state.clone();
          new_state.is_root = false;
          if element.element.attributes.contains_key(name) {
            Some(T::gen(element, new_state, Some(name))?)
          } else {
            None
          }
        }
        GenType::Content => {
          let mut new_state = gen_state.clone();
          new_state.is_root = false;
          element.try_get_child_with(name, |mut value| {
            T::gen(&mut value, new_state.clone(), Some(name))
          })?
        }
      };

      // if gen_state.is_root {
      //   element.finalize(false, false)?;
      // }

      Ok(output)
    } else {
      Err(XsdGenError {
        node_name: element.node_name(),
        ty: xsd_types::XsdType::Unknown,
        msg: format!("Expected node name to parse option got None."),
      })?
    }
  }
}

impl<T: FromXmlString> XsdGen for T {
  fn gen(
    element: &mut XMLElement,
    gen_state: GenState,
    name: Option<&str>,
  ) -> Result<Self, XsdIoError> {
    if let Some(name) = name {
      let output = match gen_state.state {
        GenType::Attribute => element.get_attribute(name),
        GenType::Content => element.get_content(),
      };

      // if gen_state.is_root {
      //   element.finalize(false, false)?;
      // }

      output
    } else {
      return Err(XsdGenError {
        node_name: element.node_name(),
        ty: xsd_types::XsdType::Unknown,
        msg: format!(
          "Expected node name to parse {} implementing FromXmlString got None.",
          std::any::type_name::<T>()
        ),
      })?;
    }
  }
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
