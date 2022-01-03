mod annotation;
mod attribute;
mod attribute_group;
mod choice;
mod complex_content;
mod complex_type;
mod element;
mod enumeration;
mod extension;
mod group;
mod import;
mod list;
mod max_occurences;
mod qualification;
mod restriction;
mod rust_types_mapping;
mod schema;
mod sequence;
mod simple_content;
mod simple_type;
mod union;
mod xsd_context;

use log::info;
use std::collections::BTreeMap;
use std::fs;
use xsd_context::XsdContext;
use yaserde::de::from_str;

#[derive(Clone, Debug)]
pub struct Xsd {
  context: XsdContext,
  schema: schema::Schema,
}

impl Xsd {
  pub fn new(
    content: &str,
    module_namespace_mappings: &BTreeMap<String, String>,
  ) -> Result<Self, String> {
    let context = XsdContext::new(content)?;
    dbg!("MADE CONTEXT");
    let context = context.with_module_namespace_mappings(module_namespace_mappings);
    dbg!("MODE");
    let schema: schema::Schema = from_str(content)?;
    dbg!("PARSED SCHEMA");

    Ok(Xsd { context, schema })
  }

  pub fn new_from_file(
    source: &str,
    module_namespace_mappings: &BTreeMap<String, String>,
  ) -> Result<Self, String> {
    dbg!("STARTING");
    let content = if source.starts_with("http://") || source.starts_with("https://") {
      info!("Load HTTP schema {}", source);
      reqwest::blocking::get(source)
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?
    } else {
      let path = std::env::current_dir().unwrap();
      info!("The current directory is {}", path.display());

      fs::read_to_string(source).map_err(|e| e.to_string())?
    };

    dbg!("HAVE FILE");

    // skip BOM header, can be present on some files
    let content = if content.as_bytes()[0..3] == [0xef, 0xbb, 0xbf] {
      content[3..].to_owned()
    } else {
      content
    };

    dbg!("SKIP BOM");

    Xsd::new(&content, module_namespace_mappings)
  }

  pub fn generate(&mut self, target_prefix: &Option<String>) -> String {
    self.schema.generate(&mut self.context)
  }
}

// #[cfg(test)]
// mod test {
//   use std::collections::BTreeMap;

//   use super::Xsd;

//   #[test]
//   fn musicxml() {
//     let mut xsd = Xsd::new_from_file(
//       "C:/Users/micro/Code/musicxml-rs/assets/musicxml.xsd",
//       &BTreeMap::new(),
//     )
//     .unwrap();
//     let output = xsd.generate(&None);

//     dbg!(output);
//   }
// }
