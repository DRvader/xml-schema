use crate::codegen::Field;

use super::{
  annotation::Annotation,
  choice::Choice,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{to_field_name, XsdContext, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Group {
  pub id: Option<String>,
  pub name: Option<String>,
  pub refers: Option<String>,
  pub min_occurences: u64,
  pub max_occurences: MaxOccurences,
  pub annotation: Option<Annotation>,
  pub sequence: Option<Sequence>,
  pub choice: Option<Choice>,
}

impl Group {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("group")?;

    let name = element.try_get_attribute("name")?;
    let refers = element.try_get_attribute("ref")?;

    let sequence = element.try_get_child_with("sequence", Sequence::parse)?;
    let choice = element.try_get_child_with("choice", Choice::parse)?;

    if name.is_some() && refers.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "name and ref cannot both present in {}",
        element.name()
      )));
    }

    if sequence.is_some() && choice.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "sequence and choice cannot both present in {}",
        element.name()
      )));
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
    match (&self.name, &parent_name, &self.refers) {
      (Some(name), _, None) => match (&self.choice, &self.sequence) {
        (None, Some(sequence)) => {
          let mut seq = sequence.get_implementation(
            Some(XsdName {
              namespace: None,
              local_name: name.clone(),
              ty: super::xsd_context::XsdType::Group,
            }),
            context,
          )?;
          let ty = format!("Group{}", seq.element.get_type().to_string());
          seq.element.set_type(ty.clone());

          for i in &mut seq.implementation {
            i.target = ty.clone().into();
          }

          Ok(seq)
        }
        (Some(choice), None) => {
          let mut choice = choice.get_implementation(
            Some(XsdName {
              namespace: None,
              local_name: name.clone(),
              ty: super::xsd_context::XsdType::Group,
            }),
            context,
          )?;

          let ty = format!("Group{}", choice.element.get_type().to_string());
          choice.element.set_type(ty.clone());

          for i in &mut choice.implementation {
            i.target = ty.clone().into();
          }

          Ok(choice)
        }
        _ => unreachable!("The Xsd is invalid!"),
      },
      (None, _, Some(refers)) => {
        let name = XsdName {
          namespace: None,
          local_name: refers.to_string(),
          ty: super::xsd_context::XsdType::Group,
        };
        let inner = if let Some(imp) = context.structs.get(&name) {
          imp
        } else {
          return Err(XsdError::XsdImplNotFound(name));
        };

        let field_name = if let Some(parent_name) = &parent_name {
          to_field_name(&parent_name.local_name)
        } else if let Some(field_hint) = &inner.fieldname_hint {
          field_hint.clone()
        } else {
          to_field_name(&inner.infer_type_name())
        };

        let name = if let Some(parent_name) = parent_name {
          parent_name
        } else {
          XsdName {
            namespace: None,
            local_name: inner.infer_type_name(),
            ty: super::xsd_context::XsdType::Group,
          }
        };

        Ok(XsdImpl {
          name,
          element: XsdElement::Field(
            Field::new(&field_name, inner.element.get_type())
              .vis("pub")
              .to_owned(),
          ),
          fieldname_hint: Some(field_name.to_string()),
          inner: vec![],
          implementation: vec![],
        })
      }
      _ => unreachable!("The Xsd is invalid!"),
    }
  }
}
