use super::{
  annotation::Annotation,
  choice::Choice,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{XsdContext, XsdImpl, XsdName},
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
    element.check_name("xs:group")?;

    let name = element.try_get_attribute("name")?;
    let refers = element.try_get_attribute("ref")?;

    let sequence = element.try_get_child_with("xs:sequence", |child| Sequence::parse(child))?;
    let choice = element.try_get_child_with("xs:choice", |child| Choice::parse(child))?;

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
      annotation: element.try_get_child_with("xs:annotation", |child| Annotation::parse(child))?,
      sequence,
      choice,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> XsdImpl {
    match (&self.name, parent_name, &self.refers) {
      (Some(name), _, None) => match (&self.choice, &self.sequence) {
        (None, Some(sequence)) => sequence.get_implementation(
          XsdName {
            namespace: None,
            local_name: name.clone(),
          },
          context,
        ),
        (Some(choice), None) => choice.get_implementation(
          XsdName {
            namespace: None,
            local_name: name.clone(),
          },
          context,
        ),
        _ => unreachable!("The Xsd is invalid!"),
      },
      (None, Some(name), Some(refers)) => {
        let mut inner = context
          .structs
          .get(&XsdName {
            namespace: None,
            local_name: refers.to_string(),
          })
          .unwrap()
          .clone();

        inner.element.set_type(&name.local_name);

        inner
      }
      _ => unreachable!("The Xsd is invalid!"),
    }
  }
}