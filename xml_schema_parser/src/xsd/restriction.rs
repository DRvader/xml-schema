use super::{
  xsd_context::{XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};
use crate::{
  codegen::{Block, Enum, Function, Impl, Struct, Type},
  xsd::XsdContext,
};
use heck::CamelCase;

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Restriction {
  pub base: String,
  pub min_inclusive: Option<i64>,
  pub max_inclusive: Option<i64>,
  pub min_exclusive: Option<i64>,
  pub enumerations: Vec<String>,
  pub pattern: Option<String>,
  pub min_length: Option<i64>,
}

pub enum RestrictionParentType {
  SimpleType,
  ComplexContent,
  SimpleContent,
}

impl Restriction {
  pub fn parse(
    _parent_type: RestrictionParentType,
    mut element: XMLElementWrapper,
  ) -> Result<Self, XsdError> {
    element.check_name("restriction")?;

    let output = Self {
      base: element.get_attribute("base")?,
      min_inclusive: element
        .try_get_child_with("minInclusive", |mut child| child.get_attribute("value"))?,
      max_inclusive: element
        .try_get_child_with("maxInclusive", |mut child| child.get_attribute("value"))?,
      min_exclusive: element
        .try_get_child_with("minExclusive", |mut child| child.get_attribute("value"))?,
      enumerations: element
        .get_children_with("enumeration", |mut child| child.get_attribute("value"))?,
      pattern: element.try_get_child_with("pattern", |mut child| child.get_attribute("value"))?,
      min_length: element
        .try_get_child_with("minLength", |mut child| child.get_attribute("value"))?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    _parent_type: RestrictionParentType,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let base_type = context.structs.get(&XsdName {
      namespace: None,
      local_name: self.base.clone(),
    });

    if !context.allow_unknown_type && base_type.is_none() {
      return Err(XsdError::XsdImplNotFound(XsdName {
        namespace: None,
        local_name: self.base.clone(),
      }));
    }

    let (_local, _generated_type) = if let Some(base_type) = base_type {
      (true, base_type.element.get_type())
    } else {
      (false, Type::new(&self.base.replace(":", "::")))
    };

    Ok(if !self.enumerations.is_empty() {
      let type_name = parent_name.to_struct_name();
      let mut generated_enum = Enum::new(&type_name);

      let mut value_block = Block::new("match self");
      for enumeration in &self.enumerations {
        let enum_name = enumeration.to_camel_case();
        generated_enum.new_variant(&enum_name);

        value_block.line(format!(
          "{}::{} => {}.parse(),",
          type_name, enum_name, enumeration
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
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Enum(generated_enum),
        inner: Vec::new(),
        implementation: vec![enum_impl],
      }
    } else {
      XsdImpl {
        name: Some(parent_name.clone()),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Struct(Struct::new(&parent_name.to_struct_name())),
        inner: Vec::new(),
        implementation: Vec::new(),
      }
    })
  }
}
