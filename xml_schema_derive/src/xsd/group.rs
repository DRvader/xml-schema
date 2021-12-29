use heck::{CamelCase, SnakeCase};
use proc_macro2::{Ident, Span, TokenStream};
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::{
  max_occurences::MaxOccurences, sequence::Sequence, xsd_context::XsdContext, Implementation,
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Group {
  #[yaserde(attribute)]
  pub id: Option<String>,
  #[yaserde(attribute)]
  pub name: Option<String>,
  #[yaserde(rename = "ref", attribute)]
  pub refers: Option<String>,
  #[yaserde(rename = "minOccurs", attribute)]
  pub min_occurences: Option<u64>,
  #[yaserde(rename = "maxOccurs", attribute)]
  pub max_occurences: Option<MaxOccurences>,
  pub sequence: Option<Sequence>,
}

impl Implementation for Group {
  fn implement(
    &self,
    namespace_definition: &proc_macro2::TokenStream,
    prefix: &Option<String>,
    context: &mut XsdContext,
  ) {
    if let Some(sequence) = &self.sequence {
      let rust_type = Ident::new(
        &self
          .name
          .as_ref()
          .unwrap()
          .replace(".", "_")
          .to_camel_case(),
        Span::call_site(),
      );
      sequence.implement(namespace_definition, prefix, context);
      let implement = sequence.get_field_implementation(context, prefix);
      context.groups.insert(
        (None, self.name.unwrap()),
        quote! {
          pub struct #rust_type {
            #implement
          }
        },
      );
    }
  }

  fn get_field(
    &self,
    namespace_definition: &TokenStream,
    prefix: &Option<String>,
    context: &XsdContext,
  ) -> TokenStream {
    context
      .groups
      .get(&(None, self.refers.unwrap()))
      .unwrap()
      .clone()
  }
}

impl Group {
  pub fn get_field_implementation(
    &self,
    context: &XsdContext,
    prefix: &Option<String>,
    multiple: bool,
  ) -> TokenStream {
    if self.needs_define() {
      return quote!();
    }

    let prefix_attribute = if let Some(prefix) = prefix {
      quote!(, prefix=#prefix)
    } else {
      quote!()
    };

    let rust_type = Ident::new(
      &self
        .refers
        .as_ref()
        .unwrap()
        .replace(".", "_")
        .to_camel_case(),
      Span::call_site(),
    );
    let yaserde_rename = &self.name;

    let rust_name = self.refers.as_ref().unwrap().to_snake_case();

    let rust_type = if self.min_occurences == Some(0) {
      quote!(Option<#rust_type>)
    } else {
      quote!(#rust_type)
    };

    quote! {
      #[yaserde(flatten, rename=#yaserde_rename #prefix_attribute)]
      pub #rust_name: #rust_type,
    }
  }

  pub fn needs_define(&self) -> bool {
    self.name.is_some()
  }
}
