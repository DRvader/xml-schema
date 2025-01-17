use xsd_codegen::XMLElement;
use xsd_types::XsdIoError;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Annotation {
  pub id: Option<String>,
  pub documentation: Vec<String>,
}

impl Annotation {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("annotation")?;

    let mut output = Ok(Self {
      id: element.try_get_attribute("id")?,
      documentation: element
        .get_children_with_filter("documentation", |mut child| child.try_get_content())?,
    });

    if let Ok(output) = &mut output {
      for doc in &mut output.documentation {
        *doc = doc.replace('\t', "  ");
      }
    }

    element.finalize(false, false)?;

    output
  }

  pub fn get_doc(&self) -> Vec<String> {
    self.documentation.clone()
  }
}
