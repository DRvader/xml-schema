use crate::codegen::{Block, Enum, Fields, Struct, Type, TypeDef, Variant};
use heck::{CamelCase, SnakeCase};

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Cursor;
use xml::namespace::Namespace;
use xml::reader::{EventReader, XmlEvent};

use crate::codegen::{Field, Impl};

use super::XsdError;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct XsdName {
  pub namespace: Option<String>,
  pub local_name: String,
}

impl std::fmt::Display for XsdName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(namespace) = &self.namespace {
      write!(f, "{}:{}", namespace, self.local_name)
    } else {
      write!(f, "{}", self.local_name)
    }
  }
}

impl XsdName {
  pub fn new(name: &str) -> Self {
    Self {
      namespace: None,
      local_name: name.to_string(),
    }
  }

  pub fn to_struct_name(&self) -> String {
    to_struct_name(&self.local_name)
  }

  pub fn to_field_name(&self) -> String {
    to_field_name(&self.local_name)
  }
}

pub fn to_struct_name(name: &str) -> String {
  name.replace(".", "_").to_camel_case()
}

pub fn to_field_name(name: &str) -> String {
  let name = name.to_snake_case();

  if name == "type" {
    "r#type".to_string()
  } else {
    name
  }
}

#[derive(Clone, Debug)]
pub enum XsdElement {
  Empty,
  Struct(Struct),
  Enum(Enum),
  Type(Type),
}

impl XsdElement {
  pub fn fmt(&self, f: &mut crate::codegen::Formatter) -> core::fmt::Result {
    match &self {
      XsdElement::Empty => Ok(()),
      XsdElement::Struct(r#struct) => r#struct.fmt(f),
      XsdElement::Enum(r#enum) => r#enum.fmt(f),
      XsdElement::Type(r#type) => r#type.fmt(f),
    }
  }

  pub fn get_type(&self) -> Type {
    self.try_get_type().unwrap()
  }

  pub fn try_get_type(&self) -> Option<Type> {
    match &self {
      XsdElement::Empty => None,
      XsdElement::Struct(r#struct) => Some(r#struct.ty().to_owned()),
      XsdElement::Enum(r#enum) => Some(r#enum.ty().to_owned()),
      XsdElement::Type(r#type) => Some(r#type.clone()),
    }
  }

  pub fn set_type(&mut self, name: impl Into<Type>) {
    match self {
      XsdElement::Struct(r#struct) => {
        r#struct.type_def.ty = name.into();
      }
      XsdElement::Enum(r#enum) => {
        r#enum.type_def.ty = name.into();
      }
      _ => {}
    }
  }

  pub fn is_builtin_type(&self) -> bool {
    match &self {
      XsdElement::Type(ty) => [
        "String", "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "u128", "i128", "bool",
        "f32", "f64", "char",
      ]
      .contains(&ty.name.as_str()),
      _ => false,
    }
  }
}

#[derive(Default, Debug, Clone)]
pub struct XsdDeserialize {
  map: BTreeMap<String, (Block, bool)>,
}

#[derive(Debug, Clone)]
pub struct XsdImpl {
  pub name: Option<XsdName>,
  pub fieldname_hint: Option<String>,
  pub element: XsdElement,
  pub inner: Vec<XsdImpl>,
  pub implementation: Vec<Impl>,
}

impl Default for XsdImpl {
  fn default() -> Self {
    Self {
      name: None,
      fieldname_hint: None,
      element: XsdElement::Empty,
      inner: Default::default(),
      implementation: Default::default(),
    }
  }
}

pub enum MergeType {
  Fields,
  Structs,
}

pub struct MergeSettings<'a> {
  pub conflict_prefix: Option<&'a str>,
  pub merge_type: MergeType,
}

impl<'a> MergeSettings<'a> {
  pub const ATTRIBUTE: MergeSettings<'a> = MergeSettings {
    conflict_prefix: Some("attr_"),
    merge_type: MergeType::Structs,
  };
}

impl<'a> Default for MergeSettings<'a> {
  fn default() -> Self {
    Self {
      conflict_prefix: None,
      merge_type: MergeType::Structs,
    }
  }
}

impl XsdImpl {
  pub fn fmt(&self, f: &mut crate::codegen::Formatter<'_>) -> core::fmt::Result {
    self.element.fmt(f)?;
    for r#impl in &self.implementation {
      r#impl.fmt(f)?;
    }

    Ok(())
  }

  pub fn to_string(&self) -> Result<String, core::fmt::Error> {
    let mut dst = String::new();
    let mut formatter = crate::codegen::Formatter::new(&mut dst);

    self.fmt(&mut formatter)?;

    Ok(dst)
  }

  pub fn merge_into_enum(&mut self, mut other: XsdImpl, merge_as_fields: bool) {
    other.element = match other.element {
      XsdElement::Struct(str) => {
        let mut gen_enum = Enum {
          type_def: str.type_def.clone(),
          variants: vec![],
        };

        match str.fields {
          Fields::Empty => {}
          Fields::Tuple(tup) => {
            if merge_as_fields {
              for t in tup {
                let mut variant = Variant::new(&t.1.name);
                variant.fields.tuple(t.1);
                gen_enum.push_variant(variant);
              }
            } else {
              let mut variant = Variant::new(&str.type_def.ty.name);
              for t in tup {
                variant.fields.tuple(t.1);
              }
              gen_enum.push_variant(variant);
            }
          }
          Fields::Named(names) => {
            if merge_as_fields {
              for t in names {
                let mut variant = Variant::new(&t.name);
                variant.fields.tuple(t.ty);
                gen_enum.push_variant(variant);
              }
            } else {
              let mut variant = Variant::new(&str.type_def.ty.name);
              for t in names {
                variant.fields.push_named(t);
              }
              gen_enum.push_variant(variant);
            }
          }
        }

        XsdElement::Enum(gen_enum)
      }
      _ => other.element,
    };

    self.merge(other, MergeSettings::default())
  }

  fn merge_typedef(this: &mut TypeDef, other: TypeDef) {
    match (&mut this.docs, other.docs) {
      (None, None) => {}
      (None, Some(b)) => {
        this.docs = Some(b);
      }
      (Some(_), None) => {}
      (Some(a), Some(b)) => {
        if !a.docs.ends_with('\n') {
          a.docs.push('\n');
        }
        a.docs.push_str(&b.docs);
      }
    }

    for derive in other.derive {
      if !this.derive.contains(&derive) {
        this.derive.push(derive);
      }
    }

    for allow in other.allow {
      if !this.allow.contains(&allow) {
        this.allow.push(allow);
      }
    }

    for bound in other.bounds {
      let bounded_names = bound
        .bound
        .iter()
        .map(|t| t.name.clone())
        .collect::<Vec<_>>();

      let mut add = true;
      for b in &this.bounds {
        assert!(
          b.name != bound.name
            || (b.bound.len() == bound.bound.len()
              && b.bound.iter().all(|t| bounded_names.contains(&t.name)))
        );

        if b.name == bound.name {
          add = false;
          break;
        }
      }

      if add {
        this.bounds.push(bound);
      }
    }

    for macros in other.macros {
      if !this.macros.contains(&macros) {
        this.macros.push(macros);
      }
    }
  }

  pub fn merge(&mut self, other: XsdImpl, settings: MergeSettings) {
    match &settings.merge_type {
      MergeType::Fields => self.merge_fields(other, settings),
      MergeType::Structs => self.merge_structs(other, settings),
    }
  }

  pub fn merge_structs(&mut self, other: XsdImpl, _settings: MergeSettings) {
    match &mut self.element {
      XsdElement::Empty => unimplemented!("Cannot merge {:?} into an empty struct.", other.name),
      XsdElement::Struct(a) => match other.element {
        XsdElement::Empty => {}
        XsdElement::Struct(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          a.push_field(Field::new(&field_name, b.ty()));
        }
        XsdElement::Enum(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          a.push_field(Field::new(&field_name, b.ty()));
        }
        XsdElement::Type(b) => {
          let field_name = to_field_name(other.fieldname_hint.as_ref().unwrap_or(&b.name));
          a.push_field(Field::new(&field_name, b));
        }
      },
      XsdElement::Enum(a) => match other.element {
        XsdElement::Empty => {}
        XsdElement::Struct(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          a.new_variant(&field_name).tuple(b.ty());
        }
        XsdElement::Enum(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          a.new_variant(&field_name).tuple(b.ty());
        }
        XsdElement::Type(b) => {
          let field_name = to_field_name(other.fieldname_hint.as_ref().unwrap_or(&b.name));
          a.new_variant(&field_name).tuple(b);
        }
      },
      XsdElement::Type(_) => unimplemented!("Cannot merge into type."),
    }

    self.inner.extend(other.inner);
  }

  pub fn merge_fields(&mut self, other: XsdImpl, settings: MergeSettings) {
    match (&mut self.element, other.element) {
      (XsdElement::Empty, element) => {
        self.element = element;
      }
      (_, XsdElement::Empty) => {}
      (XsdElement::Struct(a), XsdElement::Struct(b)) => {
        match (&mut a.fields, b.fields) {
          (_, Fields::Empty) => {}
          (Fields::Empty, Fields::Tuple(b)) => {
            a.fields = Fields::Tuple(b);
          }
          (Fields::Empty, Fields::Named(b)) => {
            a.fields = Fields::Named(b);
          }
          (Fields::Tuple(a), Fields::Tuple(b)) => {
            for field in b {
              for f in a.iter() {
                if field.1.name == f.1.name {
                  panic!("Merge conflict in field!");
                }
              }
              a.push(field);
            }
          }
          (Fields::Named(a), Fields::Named(b)) => {
            for mut field in b {
              let mut conflict = false;
              for f in a.iter() {
                if field.name == f.name {
                  conflict = true;
                  break;
                }
              }

              if conflict {
                if let Some(prefix) = settings.conflict_prefix {
                  field.name = format!("{}{}", prefix, field.name);
                  a.push(field)
                } else {
                  panic!("Merge conflict in field!");
                }
              } else {
                a.push(field);
              }
            }
          }
          (Fields::Tuple(_), Fields::Named(_)) => {
            panic!("[ERROR] Tried to merge named field into tuple!")
          }
          (Fields::Named(_), Fields::Tuple(_)) => {
            panic!("[ERROR] Tried to merge tuple field into named!")
          }
        }
        Self::merge_typedef(&mut a.type_def, b.type_def);
      }
      (XsdElement::Enum(a), XsdElement::Enum(b)) => {
        for variant in b.variants {
          for v in &a.variants {
            if variant.name == v.name {
              panic!("Merge conflict in variant!");
            }
          }
          a.push_variant(variant);
        }
        Self::merge_typedef(&mut a.type_def, b.type_def);
      }
      (XsdElement::Type(_a), XsdElement::Type(_b)) => {
        panic!("Tried to merge type with type");
      }
      _ => panic!("Invalid merge"),
    }

    // Check trait impls, use current on conflict (will need to change)
    //  Have special handling for known traits
    // Check normal impls, raise error on conflicting names
    // for j in other.implementation {
    //   for i in &self.implementation {
    //     i.fns.extend(j.fns);
    //     i.macros.extend(j.macros);
    //   }

    //   i.fns.extend(j.fns);
    //   i.macros.extend(j.macros);
    // }

    // Check for two things that are named the same
    for j in other.inner {
      for i in &self.inner {
        assert!(match (i.element.try_get_type(), j.element.try_get_type()) {
          (None, None) => false,
          (None, Some(_)) => true,
          (Some(_), None) => true,
          (Some(a), Some(b)) => a.name != b.name,
        });
      }
      self.inner.push(j);
    }
  }
}

#[derive(Clone, Debug)]
pub struct XsdContext {
  module_namespace_mappings: BTreeMap<String, String>,
  pub namespace: Namespace,
  xml_schema_prefix: Option<String>,
  pub groups: BTreeMap<XsdName, Vec<Field>>,
  pub structs: BTreeMap<XsdName, XsdImpl>,
  pub allow_unknown_type: bool,
}

impl XsdContext {
  pub fn new(content: &str) -> Result<Self, XsdError> {
    let cursor = Cursor::new(content);
    let parser = EventReader::new(cursor);

    for xml_element in parser {
      match xml_element {
        Ok(XmlEvent::StartElement {
          name, namespace, ..
        }) => {
          if name.namespace == Some("http://www.w3.org/2001/XMLSchema".to_string())
            && name.local_name == "schema"
          {
            let module_namespace_mappings = BTreeMap::new();
            let xml_schema_prefix = name.prefix;

            return Ok(XsdContext {
              module_namespace_mappings,
              namespace,
              xml_schema_prefix: xml_schema_prefix.clone(),
              allow_unknown_type: false,
              groups: BTreeMap::new(),
              structs: BTreeMap::from([
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}bool",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("bool")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}boolean",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("bool")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}positiveInteger",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u64")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}byte",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("i8")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}unsignedByte",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u8")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}short",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("i16")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}unsignedShort",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u16")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}int",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("i32")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}integer",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("i32")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}unsignedInt",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u32")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}long",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("i64")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}unsignedLong",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u64")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}nonNegativeInteger",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u64")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}double",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("f64")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}decimal",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("f64")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}string",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}normalizedString",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}anyURI",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}NMTOKEN",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}token",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}language",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}hexBinary",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}dateTime",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}base64Binary",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}duration",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}gYear",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("u16")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}ID",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}IDREF",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}IDREFS",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}anyType",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("String")),
                    ..Default::default()
                  },
                ),
                (
                  XsdName {
                    namespace: None,
                    local_name: format!(
                      "{}{}date",
                      xml_schema_prefix.as_deref().unwrap_or(""),
                      if xml_schema_prefix.is_some() { ":" } else { "" }
                    ),
                  },
                  XsdImpl {
                    element: XsdElement::Type(Type::new("chrono::Date")),
                    ..Default::default()
                  },
                ),
              ]),
            });
          }
        }
        Err(_) => break,
        _ => {}
      }
    }

    Err(XsdError::XsdParseError(
      "Bad XML Schema, unable to found schema element.".to_string(),
    ))
  }

  pub fn with_module_namespace_mappings(
    mut self,
    module_namespace_mappings: &BTreeMap<String, String>,
  ) -> Self {
    self.module_namespace_mappings = module_namespace_mappings.clone();
    self
  }

  pub fn has_xml_schema_prefix(&self) -> bool {
    self.xml_schema_prefix.is_some()
  }

  pub fn match_xml_schema_prefix(&self, value: &str) -> bool {
    self.xml_schema_prefix == Some(value.to_string())
  }

  pub fn get_module(&self, prefix: &str) -> Option<String> {
    self
      .namespace
      .get(prefix)
      .map(|namespace| {
        self
          .module_namespace_mappings
          .get(namespace)
          .map(|module| module.to_owned())
      })
      .unwrap_or_else(|| None)
  }
}

#[test]
fn get_module() {
  let context = XsdContext::new(
    r#"
    <xs:schema
      xmlns:xs="http://www.w3.org/2001/XMLSchema"
      xmlns:example="http://example.com"
      >
    </xs:schema>
  "#,
  )
  .unwrap();

  let mut mapping = BTreeMap::new();
  mapping.insert(
    "http://example.com".to_string(),
    "crate::example".to_string(),
  );
  let context = context.with_module_namespace_mappings(&mapping);

  assert_eq!(
    context.get_module("example"),
    Some("crate::example".to_string())
  );
  assert_eq!(context.get_module("other"), None);
}

#[test]
fn bad_schema_definition() {
  let context = XsdContext::new(
    r#"
    <xs:schema
      xmlns="http://www.w3.org/2001/XMLSchema"
      >
    </xs:schema>
  "#,
  );

  assert!(context.is_err());
}
