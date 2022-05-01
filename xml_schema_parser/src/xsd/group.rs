use xsd_codegen::XMLElement;
use xsd_types::{to_field_name, XsdIoError, XsdName, XsdParseError, XsdType};

use super::{
  annotation::Annotation,
  choice::Choice,
  general_xsdgen,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{XsdContext, XsdImpl, XsdImplType},
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Group {
  pub id: Option<String>,
  pub name: Option<XsdName>,
  pub refers: Option<XsdName>,
  pub min_occurences: u64,
  pub max_occurences: MaxOccurences,
  pub annotation: Option<Annotation>,
  pub sequence: Option<Sequence>,
  pub choice: Option<Choice>,
}

impl Group {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("group")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::Group));
    let refers = element
      .try_get_attribute("ref")?
      .map(|v: String| XsdName::new(&v, XsdType::Group));

    let sequence = element.try_get_child_with("sequence", Sequence::parse)?;
    let choice = element.try_get_child_with("choice", Choice::parse)?;

    if name.is_some() && refers.is_some() {
      return Err(
        XsdParseError {
          node_name: element.node_name(),
          msg: "name and ref cannot both present".to_string(),
        }
        .into(),
      );
    }

    if sequence.is_some() && choice.is_some() {
      return Err(
        XsdParseError {
          node_name: element.node_name(),
          msg: "sequence and choice cannot both present in".to_string(),
        }
        .into(),
      );
    }

    let output = Self {
      id: element.try_get_attribute("id")?,
      name,
      refers,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element
        .try_get_attribute("maxOccurs")?
        .unwrap_or(MaxOccurences::Number { value: 1 }),
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
      sequence,
      choice,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let gen = match (&self.name, &parent_name, &self.refers) {
      (Some(name), _, None) => match (&self.choice, &self.sequence) {
        (None, Some(sequence)) => sequence.get_implementation(Some(name.clone()), context)?,
        (Some(choice), None) => choice.get_implementation(Some(name.clone()), context)?,
        _ => unreachable!("The Xsd is invalid!"),
      },
      (None, _, Some(refers)) => {
        let inner = if let Some(imp) = context.search(refers) {
          imp
        } else {
          return Err(XsdError::XsdImplNotFound(refers.clone()));
        };

        let field_name = if let Some(parent_name) = &parent_name {
          parent_name.to_field_name()
        } else if let Some(field_hint) = &inner.fieldname_hint {
          field_hint.clone()
        } else {
          to_field_name(&inner.infer_type_name())
        };

        let mut name = if let Some(parent_name) = parent_name.clone() {
          parent_name
        } else {
          XsdName {
            namespace: None,
            local_name: inner.infer_type_name(),
            ty: XsdType::Group,
          }
        };

        name.ty = XsdType::Group;

        XsdImpl {
          name: name.clone(),
          element: XsdImplType::Type(inner.element.get_type().xml_name(Some(name))),
          fieldname_hint: Some(field_name),
          inner: vec![],
          implementation: vec![],
          flatten: true,
        }
      }
      _ => unreachable!("The Xsd is invalid!"),
    };

    let multiple = match &self.max_occurences {
      MaxOccurences::Unbounded => true,
      MaxOccurences::Number { value } => *value > 1,
    } || self.min_occurences > 1;

    let option = match &self.max_occurences {
      MaxOccurences::Unbounded => false,
      MaxOccurences::Number { value } => *value == 1 && self.min_occurences == 0,
    };

    let mut gen = if multiple {
      let mut gen = general_xsdgen(gen);

      let old_name = gen.name.clone();
      gen.name.local_name = format!("inner-{}", old_name.local_name);
      XsdImpl {
        name: old_name,
        fieldname_hint: Some(gen.fieldname_hint.clone().unwrap()),
        element: XsdImplType::Type(gen.element.get_type().wrap("Vec")),
        inner: vec![gen],
        implementation: vec![],
        flatten: false,
      }
    } else if option {
      let mut gen = general_xsdgen(gen);

      let old_name = gen.name.clone();
      gen.name.local_name = format!("inner-{}", old_name.local_name);
      XsdImpl {
        name: old_name,
        fieldname_hint: Some(gen.fieldname_hint.clone().unwrap()),
        element: XsdImplType::Type(gen.element.get_type().wrap("Option")),
        inner: vec![gen],
        implementation: vec![],
        flatten: false,
      }
    } else {
      gen
    };

    if let Some(annotation) = &self.annotation {
      gen.element.add_doc(&annotation.get_doc().join("\n"));
    }

    gen.name.ty = XsdType::Group;

    Ok(gen)
  }
}
