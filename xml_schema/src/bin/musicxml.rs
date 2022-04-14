use std::collections::BTreeMap;

use xml_schema_parser::{Xsd, XsdError};

fn main() -> Result<(), XsdError> {
  // tracing_subscriber::util::SubscriberInitExt::try_init(
  //   tracing_subscriber::fmt::SubscriberBuilder::default()
  //     .with_max_level(tracing::Level::TRACE)
  //     // .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
  //     .finish(),
  // )
  // .unwrap();

  let mut xsd = Xsd::new_from_file("./xml.xsd")?;
  let output = xsd.generate(&None);

  match output {
    Err(output) => match output {
      XsdError::XsdParseError(msg) => {
        println!("{msg}");
        panic!();
      }
      output => return Err(output),
    },
    Ok(output) => {
      // dbg!(output);
    }
  }

  Ok(())
}
