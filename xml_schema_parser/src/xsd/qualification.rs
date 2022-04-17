use xsd_codegen::FromXmlString;

#[derive(Clone, Debug, PartialEq)]
pub enum Qualification {
  Qualidified,
  Unqualified,
}

impl Default for Qualification {
  fn default() -> Self {
    Qualification::Unqualified
  }
}

impl FromXmlString for Qualification {
  fn from_xml(s: &str) -> Result<Self, String> {
    match s {
      "qualified" => Ok(Qualification::Qualidified),
      "unqualified" => Ok(Qualification::Unqualified),
      err => Err(format!(
        "{} is not a valid value for qualified|unqualified",
        err
      )),
    }
  }
}
