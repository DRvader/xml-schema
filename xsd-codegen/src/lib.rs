mod codegen_helper;
mod rust_codegen;
mod xml_element;

use std::{
  collections::BTreeMap,
  ops::{Deref, DerefMut},
};

pub use rust_codegen::{
  Block, Enum, Field, Fields, Formatter, Function, Impl, Item, Module, Struct, TupleField, Type,
  TypeAlias, TypeDef, Variant,
};
pub use xml_element::XMLElement;
use xsd_types::{XsdGenError, XsdIoError};

pub use codegen_helper::{fromxml_impl, xsdgen_impl};

#[derive(Default)]
pub struct TypeStore {
  names: BTreeMap<String, usize>,
}

impl TypeStore {
  pub fn get(&mut self, name: &str) -> usize {
    let current_len = self.names.len();
    *self.names.entry(name.to_string()).or_insert(current_len)
  }
}

#[derive(Clone, Copy)]
pub enum GenType {
  Attribute,
  Content,
}

#[derive(Clone)]
pub struct GenState {
  pub is_root: bool,
  pub state: GenType,
}

impl GenState {
  pub fn to_attr(&self) -> Self {
    Self {
      is_root: self.is_root,
      state: GenType::Attribute,
    }
  }
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
        vec![T::gen(element, gen_state, name)?]
      }
      GenType::Content => {
        if let Some(name) = name {
          let mut new_state = gen_state;
          new_state.is_root = false;
          element.get_children_with(name, |mut value| {
            T::gen(&mut value, new_state.clone(), None)
          })?
        } else {
          let mut output = vec![];

          let mut last_element = element.clone();
          while let Ok(value) = T::gen(element, gen_state.clone(), None) {
            if element == &mut last_element {
              break;
            }
            output.push(value);
            last_element = element.clone();
          }
          *element = last_element;

          output
        }
      }
    };

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
          let mut new_state = gen_state;
          new_state.is_root = false;
          if element.element.attributes.contains_key(name) {
            Some(T::gen(element, new_state, Some(name))?)
          } else {
            None
          }
        }
        GenType::Content => {
          let mut new_state = gen_state;
          new_state.is_root = false;
          element.try_get_child_with(name, |mut value| {
            T::gen(&mut value, new_state.clone(), None)
          })?
        }
      };

      Ok(output)
    } else {
      let mut output = None;

      let mut last_element = element.clone();
      if let Ok(value) = T::gen(element, gen_state, None) {
        output = Some(value);
        last_element = element.clone();
      }
      *element = last_element;

      Ok(output)
    }
  }
}

impl<T: FromXmlString> XsdGen for T {
  fn gen(
    element: &mut XMLElement,
    gen_state: GenState,
    name: Option<&str>,
  ) -> Result<Self, XsdIoError> {
    match gen_state.state {
      GenType::Attribute => {
        if let Some(name) = name {
          element.get_attribute(name)
        } else {
          return Err(
            XsdGenError {
              node_name: element.node_name(),
              ty: xsd_types::XsdType::Unknown,
              msg: format!(
                "Expected node name to parse {} attribute implementing FromXmlString got None.",
                std::any::type_name::<T>()
              ),
            }
            .into(),
          );
        }
      }
      GenType::Content => {
        if let Some(name) = name {
          element.get_child_with(name, |mut element| element.get_content())
        } else if let Ok(content) = element.get_content() {
          Ok(content)
        } else if let Ok(content) = T::from_xml("") {
          Ok(content)
        } else {
          Err(XsdIoError::XsdParseError(xsd_types::XsdParseError {
            node_name: element.node_name(),
            msg: "failed to convert text to T".to_string(),
          }))
        }
      }
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

#[derive(Clone, Debug, PartialEq)]
pub struct RestrictedVec<T, const MIN: usize, const MAX: usize>(Vec<T>);

impl<T, const MIN: usize, const MAX: usize> Deref for RestrictedVec<T, MIN, MAX> {
  type Target = Vec<T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T, const MIN: usize, const MAX: usize> DerefMut for RestrictedVec<T, MIN, MAX> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<T, const MIN: usize, const MAX: usize> IntoIterator for RestrictedVec<T, MIN, MAX> {
  type Item = <Vec<T> as IntoIterator>::Item;
  type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
  }
}

impl<T: XsdGen, const MIN: usize, const MAX: usize> XsdGen for RestrictedVec<T, MIN, MAX> {
  fn gen(
    element: &mut XMLElement,
    gen_state: GenState,
    name: Option<&str>,
  ) -> Result<Self, XsdIoError> {
    let gen = <Vec<T> as XsdGen>::gen(element, gen_state, name)?;
    if gen.len() < MIN {
      return Err(XsdIoError::XsdParseError(xsd_types::XsdParseError {
        node_name: element.node_name(),
        msg: format!(
          "Generated vector length is less than the minimum size ({} < {MIN})",
          gen.len()
        ),
      }));
    }

    if MAX != 0 && gen.len() > MAX {
      return Err(XsdIoError::XsdParseError(xsd_types::XsdParseError {
        node_name: element.node_name(),
        msg: format!(
          "Generated vector length is greater than the maximuim size ({} > {MAX})",
          gen.len()
        ),
      }));
    }

    Ok(Self(gen))
  }
}

gen_simple_parse_from_xml_string!(isize);
gen_simple_parse_from_xml_string!(usize);
gen_simple_parse_from_xml_string!(i64);
gen_simple_parse_from_xml_string!(u64);
gen_simple_parse_from_xml_string!(i32);
gen_simple_parse_from_xml_string!(u32);
gen_simple_parse_from_xml_string!(i8);
gen_simple_parse_from_xml_string!(u8);
gen_simple_parse_from_xml_string!(f32);
gen_simple_parse_from_xml_string!(f64);

#[derive(PartialEq, Debug, Clone)]
pub struct Date {
  pub value: chrono::NaiveDate,
  pub timezone: Option<chrono::FixedOffset>,
}

pub fn parse_timezone(s: &str) -> Result<chrono::FixedOffset, String> {
  if s == "Z" {
    return Ok(chrono::FixedOffset::east(0));
  }

  let tokens: Vec<&str> = s[1..].split(':').collect();
  if tokens.len() != 2 || tokens[0].len() != 2 || tokens[1].len() != 2 {
    return Err("bad timezone format".to_string());
  }
  if !tokens.iter().all(|t| t.chars().all(|c| c.is_digit(10))) {
    return Err("bad timezone format".to_string());
  }

  let hours = tokens[0].parse::<i32>().unwrap();
  let minutes = tokens[1].parse::<i32>().unwrap();

  if hours > 14 || (hours == 14 && minutes != 0) || minutes >= 60 {
    return Err("bad timezone format: out of range".to_string());
  }

  let offset_secs = 60 * (60 * hours + minutes);
  match s.chars().next().unwrap() {
    '+' => Ok(chrono::FixedOffset::east(offset_secs)),
    '-' => Ok(chrono::FixedOffset::west(offset_secs)),
    _ => Err("bad timezone format: timezone should start with '+' or '-'".to_string()),
  }
}

impl FromXmlString for Date {
  fn from_xml(string: &str) -> Result<Self, String> {
    fn parse_naive_date(s: &str) -> Result<chrono::NaiveDate, String> {
      chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| e.to_string())
    }

    if let Some(s) = string.strip_suffix('Z') {
      return Ok(Date {
        value: parse_naive_date(s)?,
        timezone: Some(chrono::FixedOffset::east(0)),
      });
    }

    if string.contains('+') {
      if string.matches('+').count() > 1 {
        return Err("bad date format".to_string());
      }

      let idx: usize = string.match_indices('+').collect::<Vec<_>>()[0].0;
      let date_token = &string[..idx];
      let tz_token = &string[idx..];
      return Ok(Date {
        value: parse_naive_date(date_token)?,
        timezone: Some(parse_timezone(tz_token)?),
      });
    }

    if string.matches('-').count() == 3 {
      let idx: usize = string.match_indices('-').collect::<Vec<_>>()[2].0;
      let date_token = &string[..idx];
      let tz_token = &string[idx..];
      return Ok(Date {
        value: parse_naive_date(date_token)?,
        timezone: Some(parse_timezone(tz_token)?),
      });
    }

    Ok(Date {
      value: parse_naive_date(string)?,
      timezone: None,
    })
  }
}
