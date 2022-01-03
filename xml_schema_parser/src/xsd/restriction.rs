use crate::{
  codegen::{Block, Enum, Function, Impl, Struct, Type},
  xsd::XsdContext,
};
use heck::CamelCase;
use log::debug;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::{
  enumeration::Enumeration,
  xsd_context::{XsdElement, XsdImpl, XsdName},
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Restriction {
  #[yaserde(attribute)]
  pub base: String,
  #[yaserde(rename = "enumeration")]
  pub enumerations: Vec<Enumeration>,
}

pub enum RestrictionParentType {
  SimpleType,
  ComplexContent,
  SimpleContent,
}

impl Restriction {
  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    parent_type: RestrictionParentType,
    context: &mut XsdContext,
  ) -> XsdImpl {
    let base_type = context.structs.get(&XsdName {
      namespace: None,
      local_name: self.base.clone(),
    });

    if !context.allow_unknown_type && base_type.is_none() {
      panic!("Unknown type {}", self.base);
    }

    let (local, generated_type) = if let Some(base_type) = base_type {
      (true, base_type.element.get_type())
    } else {
      (false, Type::new(&self.base.replace(":", "::")))
    };

    if !self.enumerations.is_empty() {
      let type_name = parent_name.to_struct_name();
      let mut generated_enum = Enum::new(&type_name);

      let mut value_block = Block::new("match self");
      for enumeration in &self.enumerations {
        let enum_name = enumeration.value.to_camel_case();
        generated_enum.new_variant(&enum_name);

        value_block.line(format!(
          "{}::{} => {}.parse(),",
          type_name, enum_name, enumeration.value
        ));
      }

      let value = Function::new("value")
        .arg_ref_self()
        .ret(&parent_name.to_struct_name())
        .push_block(value_block)
        .to_owned();
      let enum_impl = Impl::new(generated_enum.ty()).push_fn(value).to_owned();

      XsdImpl {
        name: Some(parent_name.clone()),
        element: XsdElement::Enum(generated_enum),
        inner: Vec::new(),
        implementation: vec![enum_impl],
      }
    } else {
      XsdImpl {
        name: Some(parent_name.clone()),
        element: XsdElement::Struct(Struct::new(&parent_name.to_struct_name())),
        inner: Vec::new(),
        implementation: Vec::new(),
      }
    }
  }
}
