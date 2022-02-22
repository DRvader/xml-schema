use std::str::FromStr;

use super::{
  annotation::Annotation,
  attribute::Attribute,
  attribute_group::AttributeGroup,
  choice::Choice,
  group::Group,
  sequence::Sequence,
  xsd_context::{to_struct_name, MergeSettings, XsdElement, XsdImpl, XsdName, XsdType},
  XMLElementWrapper, XsdError,
};
use crate::{
  codegen::{Block, Enum, Function, Impl, Struct, Type},
  xsd::XsdContext,
};
use heck::CamelCase;

#[derive(Clone, Debug, PartialEq)]
pub enum Whitespace {
  // No normalization is done, the value is not changed (this is the behavior required by [XML 1.0 (Second Edition)] for element content)
  Preserve,
  // All occurrences of #x9 (tab), #xA (line feed) and #xD (carriage return) are replaced with #x20 (space)
  Replace,
  // After the processing implied by replace, contiguous sequences of #x20's are collapsed to a single #x20, and leading and trailing #x20's are removed.
  Collapse,
}

impl Default for Whitespace {
  fn default() -> Self {
    Self::Preserve
  }
}

impl FromStr for Whitespace {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "preserve" => Ok(Self::Preserve),
      "replace" => Ok(Self::Replace),
      "collapse" => Ok(Self::Collapse),
      s => Err(format!(
        "{s} is not a recognized whitespace value; expected (preserve|replace|collapse)."
      )),
    }
  }
}

// TODO(drosen): Actually implement these checks on the input

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Restriction {
  pub base: String,
  pub min_inclusive: Option<i64>,
  pub max_inclusive: Option<i64>,
  pub min_exclusive: Option<i64>,
  pub max_exclusive: Option<i64>,
  pub total_digits: Option<i64>,
  pub fraction_digits: Option<i64>,

  pub enumerations: Vec<String>,
  pub pattern: Option<String>,
  pub whitespace: Option<Whitespace>,

  pub length: Option<i64>,
  pub min_length: Option<i64>,
  pub max_length: Option<i64>,

  pub annotation: Option<Annotation>,

  pub choice: Option<Choice>,
  pub group: Option<Group>,
  pub sequence: Option<Sequence>,

  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
}

#[derive(Debug)]
pub enum RestrictionParentType {
  SimpleType,
  ComplexContent,
  SimpleContent,
}

impl Restriction {
  pub fn parse(
    parent_type: RestrictionParentType,
    mut element: XMLElementWrapper,
  ) -> Result<Self, XsdError> {
    element.check_name("restriction")?;

    let annotation = element.try_get_child_with("annotation", Annotation::parse)?;
    let attributes = element.get_children_with("attribute", Attribute::parse)?;
    let attribute_groups = element.get_children_with("attributeGroup", AttributeGroup::parse)?;

    let choice = element.try_get_child_with("choice", Choice::parse)?;
    let group = element.try_get_child_with("group", Group::parse)?;
    let sequence = element.try_get_child_with("sequence", Sequence::parse)?;

    match parent_type {
      RestrictionParentType::SimpleType => {
        if choice.is_some()
          || group.is_some()
          || sequence.is_some()
          || !attributes.is_empty()
          || !attribute_groups.is_empty()
        {
          return Err(XsdError::XsdParseError(format!(
            "choice | group | sequence | attribute | attributeGroup cannot be present in {} when the parent is a simple type.",
            element.name()
          )));
        }
      }
      RestrictionParentType::ComplexContent => {
        if choice.is_some() as u8 + group.is_some() as u8 + sequence.is_some() as u8 > 1 {
          return Err(XsdError::XsdParseError(format!(
            "choice | group | sequence may be present in {} when the parent is complex content.",
            element.name()
          )));
        }
      }
      RestrictionParentType::SimpleContent => {
        if choice.is_some() || group.is_some() || sequence.is_some() {
          return Err(XsdError::XsdParseError(format!(
            "choice | group | sequence cannot be present in {} when the parent is a simple content.",
            element.name()
          )));
        }
      }
    }

    let output = Self {
      base: element.get_attribute("base")?,
      annotation,
      min_inclusive: element
        .try_get_child_with("minInclusive", |mut child| child.get_attribute("value"))?,
      max_inclusive: element
        .try_get_child_with("maxInclusive", |mut child| child.get_attribute("value"))?,
      min_exclusive: element
        .try_get_child_with("minExclusive", |mut child| child.get_attribute("value"))?,
      max_exclusive: element
        .try_get_child_with("maxExclusive", |mut child| child.get_attribute("value"))?,
      total_digits: element
        .try_get_child_with("totalDigits", |mut child| child.get_attribute("value"))?,
      fraction_digits: element
        .try_get_child_with("fractionDigits", |mut child| child.get_attribute("value"))?,
      enumerations: element
        .get_children_with("enumeration", |mut child| child.get_attribute("value"))?,
      pattern: element.try_get_child_with("pattern", |mut child| child.get_attribute("value"))?,
      length: element.try_get_child_with("length", |mut child| child.get_attribute("value"))?,
      min_length: element
        .try_get_child_with("minLength", |mut child| child.get_attribute("value"))?,
      max_length: element
        .try_get_child_with("maxLength", |mut child| child.get_attribute("value"))?,
      whitespace: element
        .try_get_child_with("whitespace", |mut child| child.get_attribute("value"))?,

      attributes,
      attribute_groups,

      choice,
      group,
      sequence,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  fn get_simple_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
    allow_attributes: bool,
  ) -> Result<XsdImpl, XsdError> {
    let base_type = context.structs.get(&XsdName {
      namespace: None,
      local_name: self.base.clone(),
      ty: super::xsd_context::XsdType::SimpleType,
    });

    if !context.allow_unknown_type && base_type.is_none() {
      return Err(XsdError::XsdImplNotFound(XsdName {
        namespace: None,
        local_name: self.base.clone(),
        ty: super::xsd_context::XsdType::SimpleType,
      }));
    }

    let base_type = base_type.unwrap();

    let mut generated_impl = if !self.enumerations.is_empty() {
      let typename = parent_name.to_struct_name();
      let mut generated_enum = Enum::new(&typename).vis("pub").to_owned();
      for derive in ["Clone", "Debug", "PartialEq"] {
        generated_enum.derive(derive);
      }

      let mut value = Function::new("parse")
        .arg("mut element", "XMLElementWrapper")
        .ret(format!("Result<{}, XsdError>", &typename))
        .to_owned();

      let mut parse_match = Block::new("let output = match element.get_content()?");
      for enumeration in &self.enumerations {
        let enumeration = if enumeration.len() == 0 {
          "empty"
        } else {
          enumeration
        };

        let enum_name = to_struct_name(enumeration);
        generated_enum.new_variant(&enum_name);

        parse_match.line(format!("\"{}\" => Self::{},", enumeration, enum_name));
      }
      parse_match.after(";");
      value.push_block(parse_match);

      value.line("element.finalize(false, false)?;");
      value.line("Ok(output)");

      let enum_impl = Impl::new(generated_enum.ty()).push_fn(value).to_owned();

      XsdImpl {
        name: parent_name.clone(),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Enum(generated_enum),
        inner: Vec::new(),
        implementation: vec![enum_impl],
      }
    } else {
      let typename = parent_name.to_struct_name();
      let mut generated_struct = Struct::new(&typename);
      for derive in ["Clone", "Debug", "Default", "PartialEq"] {
        generated_struct.derive(derive);
      }

      let mut value = Function::new("parse")
        .arg("mut element", "XMLElementWrapper")
        .ret(format!("Result<{}, XsdError>", &typename))
        .to_owned();

      value.line("let output = Self(element.get_content()?);");
      value.line("element.finalize(false, false)?;");
      value.line("Ok(output)");

      let struct_impl = Impl::new(generated_struct.ty()).push_fn(value).to_owned();

      XsdImpl {
        name: parent_name.clone(),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Struct(
          Struct::new(&parent_name.to_struct_name())
            .tuple_field(base_type.element.get_type())
            .to_owned(),
        ),
        inner: Vec::new(),
        implementation: vec![struct_impl],
      }
    };

    if allow_attributes {
      for attribute in &self.attributes {
        generated_impl.merge(
          attribute.get_implementation(context)?,
          MergeSettings::ATTRIBUTE,
        );
      }

      for group in &self.attribute_groups {
        generated_impl.merge(
          group.get_implementation(Some(parent_name.clone()), context)?,
          MergeSettings::default(),
        );
      }
    }

    Ok(generated_impl)
  }

  fn get_complex_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let base_type = context.structs.get(&XsdName {
      namespace: None,
      local_name: self.base.clone(),
      ty: super::xsd_context::XsdType::ComplexType,
    });

    if !context.allow_unknown_type && base_type.is_none() {
      return Err(XsdError::XsdImplNotFound(XsdName {
        namespace: None,
        local_name: self.base.clone(),
        ty: super::xsd_context::XsdType::ComplexType,
      }));
    }

    let mut base_type = base_type.unwrap().clone();
    base_type.name = parent_name.clone();

    match (&self.group, &self.choice, &self.sequence) {
      (Some(group), None, None) => {
        base_type.merge(
          group.get_implementation(Some(parent_name), context)?,
          MergeSettings::default(),
        );
      }
      (None, Some(choice), None) => {
        base_type.merge(
          choice.get_implementation(Some(parent_name), context)?,
          MergeSettings::default(),
        );
      }
      (None, None, Some(sequence)) => {
        base_type.merge(
          sequence.get_implementation(Some(parent_name), context)?,
          MergeSettings::default(),
        );
      }
      _ => unreachable!("Should have already validated the input schema."),
    }

    Ok(base_type)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    parent_type: RestrictionParentType,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let mut gen = match parent_type {
      RestrictionParentType::SimpleType => {
        self.get_simple_implementation(parent_name, context, false)
      }
      RestrictionParentType::ComplexContent => {
        self.get_simple_implementation(parent_name, context, true)
      }
      RestrictionParentType::SimpleContent => self.get_complex_implementation(parent_name, context),
    }?;

    gen.name.ty = XsdType::Restriction;

    Ok(gen)
  }
}
