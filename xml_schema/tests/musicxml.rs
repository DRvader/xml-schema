use std::collections::BTreeMap;

use xml_schema_parser::{Xsd, XsdError};

#[test]
fn musicxml() -> Result<(), XsdError> {
  // tracing_subscriber::util::SubscriberInitExt::try_init(
  //   tracing_subscriber::fmt::SubscriberBuilder::default()
  //     .with_max_level(tracing::Level::TRACE)
  //     // .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
  //     .finish(),
  // )
  // .unwrap();

  let mut xsd = Xsd::new_from_file("../musicxml.xsd", &BTreeMap::new())?;
  let output = xsd.generate(&None)?;

  dbg!(output);

  Ok(())
}
