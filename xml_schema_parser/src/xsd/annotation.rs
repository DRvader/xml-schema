use super::{XMLElementWrapper, XsdError};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//     rename = "annotation"
//     prefix = "xs",
//     namespace = "xs: http://www.w3.org/2001/XMLSchema"
//   )]
pub struct Annotation {
  pub id: Option<String>,
  // #[yaserde(
  //     rename = "documentation"
  //     prefix = "xs",
  //     namespace = "xs: http://www.w3.org/2001/XMLSchema"
  //   )]
  pub documentation: Vec<String>,
}

impl Annotation {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("annotation")?;

    let output = Ok(Self {
      id: element.try_get_attribute("id")?,
      documentation: element
        .get_children_with_filter("documentation", |mut child| child.try_get_content())?,
    });

    element.finalize(false, false)?;

    output
  }

  pub fn get_doc(&self) -> Vec<String> {
    return self.documentation.clone();
  }
}
