use std::str::FromStr;

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

impl FromStr for Qualification {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
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

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn default_qualification() {
    assert_eq!(Qualification::default(), Qualification::Unqualified);
  }
}
