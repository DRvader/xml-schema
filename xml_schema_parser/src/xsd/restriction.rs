use xsd_codegen::{
  xsdgen_impl, Block, Enum, FromXmlString, Function, Impl, Struct, Variant, XMLElement,
};
use xsd_types::{to_struct_name, XsdIoError, XsdName, XsdParseError, XsdType};

use super::{
  annotation::Annotation,
  attribute::Attribute,
  attribute_group::AttributeGroup,
  choice::Choice,
  general_xsdgen,
  group::Group,
  sequence::Sequence,
  xsd_context::{MergeSettings, XsdElement, XsdImpl},
  XsdError,
};
use crate::xsd::XsdContext;

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

impl FromXmlString for Whitespace {
  fn from_xml(s: &str) -> Result<Self, String> {
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

#[derive(Clone, Debug, PartialEq)]
pub struct Restriction {
  pub base: XsdName,
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
    mut element: XMLElement,
  ) -> Result<Self, XsdIoError> {
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
          return Err(XsdParseError {
            node_name: element.node_name(),
            msg: format!(
            "choice | group | sequence | attribute | attributeGroup cannot be present in node when the parent is a simple type.",
          )})?;
        }
      }
      RestrictionParentType::ComplexContent => {
        if choice.is_some() as u8 + group.is_some() as u8 + sequence.is_some() as u8 > 1 {
          return Err(XsdParseError {
            node_name: element.node_name(),
            msg: format!(
              "choice | group | sequence may be present in node when the parent is complex content.",
            ),
          })?;
        }
      }
      RestrictionParentType::SimpleContent => {
        if choice.is_some() || group.is_some() || sequence.is_some() {
          return Err(XsdParseError {
            node_name: element.node_name(),
            msg: format!(
            "choice | group | sequence cannot be present in node when the parent is a simple content.",
          )})?;
        }
      }
    }

    let base: String = element.get_attribute("base")?;
    let output = Self {
      base: element.new_name(&base, XsdType::SimpleType),
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
    let base_type = context.search(&self.base);

    let mut generate_xsdgen = true;

    if base_type.is_none() {
      return Err(XsdError::XsdImplNotFound(self.base.clone()));
    }

    let base_type = base_type.unwrap();

    let mut generated_impl = if !self.enumerations.is_empty() {
      let typename = parent_name.to_struct_name();
      let mut generated_enum =
        Enum::new(None, &typename)
          .vis("pub")
          .derives(&["Clone", "Debug", "PartialEq"]);

      let mut value = Block::default();

      let mut parse_match =
        Block::new("let output = match element.get_content::<String>()?.as_str()");
      for enumeration in &self.enumerations {
        let enum_name = if enumeration.is_empty() {
          "Empty".to_string()
        } else {
          to_struct_name(enumeration)
        };
        generated_enum = generated_enum.push_variant(Variant::new(None, &enum_name));

        parse_match = parse_match.line(format!("\"{}\" => Self::{},", enumeration, enum_name));
      }
      parse_match = parse_match
        .push_block(
          Block::new("value => ").push_block(
            Block::new("return Err(XsdGenError")
              .line("node_name: element.name().to_string(),")
              .line("msg: format!(\"Invalid xml node found unexpected content {value}.\"),")
              .line("ty: XsdType::Restriction,")
              .after(")?"),
          ),
        )
        .after(";");
      value = value.push_block(parse_match).line("Ok(output)");

      let enum_impl = xsdgen_impl(generated_enum.ty().clone(), value);

      generate_xsdgen = false;

      XsdImpl {
        name: parent_name.clone(),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Enum(generated_enum),
        inner: Vec::new(),
        implementation: vec![enum_impl],
      }
    } else {
      XsdImpl {
        name: parent_name.clone(),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Struct(
          Struct::new(Some(parent_name.clone()), &parent_name.to_struct_name())
            .tuple_field(base_type.element.get_type())
            .derives(&["Clone", "Debug", "PartialEq"]),
        ),
        inner: Vec::new(),
        implementation: vec![],
      }
    };

    if allow_attributes {
      for attribute in &self.attributes {
        generated_impl.merge(
          attribute.get_implementation(context, false)?,
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

    let generated_impl = if generate_xsdgen {
      general_xsdgen(generated_impl)
    } else {
      generated_impl
    };

    Ok(generated_impl)
  }

  fn get_complex_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let base_type = context.search(&self.base);

    if base_type.is_none() {
      return Err(XsdError::XsdImplNotFound(self.base.clone()));
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

    Ok(general_xsdgen(base_type))
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
