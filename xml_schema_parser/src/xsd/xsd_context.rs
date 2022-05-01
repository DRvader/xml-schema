use xsd_codegen::{
  Enum, Field, Fields, Formatter, Impl, Item, Module, Struct, TupleField, Type, Variant,
};
use xsd_types::{to_field_name, to_struct_name, XsdIoError, XsdName, XsdParseError, XsdType};

use std::collections::BTreeMap;
use std::fmt::{Debug, Write};
use std::io::Cursor;
use std::iter::FromIterator;
use xml::namespace::Namespace;
use xml::reader::{EventReader, XmlEvent};

use super::XsdError;

#[derive(Clone, Debug, PartialEq)]
pub enum XsdElement {
  Struct(Struct),
  Enum(Enum),
  Field(Field),
  Type(Type),
  TypeAlias(Type, Type),
}

impl XsdElement {
  pub fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    match &self {
      XsdElement::Struct(r#struct) => r#struct.fmt(f),
      XsdElement::Enum(r#enum) => r#enum.fmt(f),
      XsdElement::TypeAlias(alias, r#type) => {
        write!(f, "pub type ")?;
        alias.fmt(f)?;
        write!(f, " = ")?;
        r#type.fmt(f)?;
        writeln!(f, ";")?;
        Ok(())
      }
      XsdElement::Field(_) => unreachable!("Should have packed field into an enum or struct."),
      _ => Ok(()),
    }
  }

  pub fn get_type(&self) -> Type {
    self.try_get_type().unwrap()
  }

  pub fn try_get_type(&self) -> Option<Type> {
    match &self {
      XsdElement::Struct(r#struct) => Some(r#struct.ty().to_owned()),
      XsdElement::Enum(r#enum) => Some(r#enum.ty().to_owned()),
      XsdElement::Type(r#type) | XsdElement::TypeAlias(r#type, _) => Some(r#type.clone()),
      XsdElement::Field(field) => Some(field.ty.clone()),
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

  pub fn add_doc(&mut self, doc: &str) {
    match self {
      XsdElement::Struct(r#struct) => {
        r#struct.doc(doc);
      }
      XsdElement::Enum(r#enum) => {
        r#enum.doc(doc);
      }
      XsdElement::Field(field) => {
        field.doc(vec![doc]);
      }
      _ => {}
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XsdImpl {
  pub name: XsdName,
  pub fieldname_hint: Option<String>,
  pub element: XsdElement,
  pub inner: Vec<XsdImpl>,
  pub implementation: Vec<Impl>,
  pub flatten: bool,
}

pub enum MergeType {
  Field,
  Attribute,
}

pub struct MergeSettings<'a> {
  pub conflict_prefix: Option<&'a str>,
  pub merge_type: MergeType,
}

impl<'a> MergeSettings<'a> {
  pub const ATTRIBUTE: MergeSettings<'a> = MergeSettings {
    conflict_prefix: Some("attr_"),
    merge_type: MergeType::Attribute,
  };
}

impl<'a> Default for MergeSettings<'a> {
  fn default() -> Self {
    Self {
      conflict_prefix: None,
      merge_type: MergeType::Field,
    }
  }
}

pub fn infer_type_name(this: &[XsdImpl]) -> String {
  let mut output = String::new();

  for i in this {
    if let Some(hint) = &i.fieldname_hint {
      output.push_str(hint);
    } else {
      output.push_str(&i.element.get_type().name);
    }
  }

  output
}

impl XsdImpl {
  fn wrap_inner_mod(&self, existing_module: &mut Module, level: usize) {
    if self.inner.is_empty() {
      return;
    }

    let mod_name = to_field_name(&self.element.get_type().name);
    let module = existing_module.get_or_new_module(&mod_name);

    module.import(
      &(0..level).map(|_| "super").collect::<Vec<_>>().join("::"),
      "*",
    );

    for inner in &self.inner {
      match &inner.element {
        XsdElement::Struct(a) => {
          module.push_struct(a.clone());
        }
        XsdElement::Enum(a) => {
          module.push_enum(a.clone());
        }
        XsdElement::Field(_) => {}
        XsdElement::Type(_) => {}
        XsdElement::TypeAlias(..) => {}
      }

      for i in &inner.implementation {
        module.push_impl(i.clone());
      }

      inner.wrap_inner_mod(module, level + 1);
    }
  }

  pub fn wrap_inner(&self) -> Option<Module> {
    if self.inner.is_empty() {
      return None;
    }

    let mut top_level = Module::new("-temp");
    self.wrap_inner_mod(&mut top_level, 1);

    for i in top_level.scope.items {
      if let Item::Module(m) = i {
        return Some(m);
      };
    }

    unreachable!();
  }

  pub fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    self.element.fmt(f)?;
    for r#impl in &self.implementation {
      r#impl.fmt(f)?;
    }

    if let Some(module) = self.wrap_inner() {
      module.fmt(f)?;
    }

    Ok(())
  }

  pub fn infer_type_name(&self) -> String {
    match &self.element {
      XsdElement::Struct(a) => match &a.fields {
        Fields::Empty => unimplemented!(),
        Fields::Tuple(tup) => tup.iter().map(|v| v.ty.name.as_str()).collect::<String>(),
        Fields::Named(names) => names
          .iter()
          .map(|f| to_struct_name(&f.name))
          .collect::<String>(),
      },
      XsdElement::Enum(a) => a.variants.iter().map(|v| v.name.as_str()).collect(),
      XsdElement::Type(ty) | XsdElement::TypeAlias(ty, _) => ty.name.clone(),
      XsdElement::Field(ty) => ty.ty.name.clone(),
    }
  }

  pub fn to_string(&self) -> Result<String, core::fmt::Error> {
    let mut dst = String::new();
    let mut formatter = Formatter::new(&mut dst);

    self.fmt(&mut formatter)?;

    Ok(dst)
  }

  pub fn to_field(&self) -> XsdImpl {
    if let XsdElement::Field(_) = self.element {
      self.clone()
    } else {
      XsdImpl {
        name: self.name.clone(),
        fieldname_hint: self.fieldname_hint.clone(),
        element: XsdElement::Field(
          Field::new(
            self.element.get_type().xml_name,
            &self
              .fieldname_hint
              .clone()
              .unwrap_or_else(|| self.name.to_field_name()),
            self.element.get_type(),
            false,
            false,
          )
          .vis("pub"),
        ),
        inner: vec![],
        implementation: vec![],
        flatten: self.flatten,
      }
    }
  }

  fn merge_inner(&mut self, others: Vec<XsdImpl>) {
    'outer: for mut other in others {
      for i in &self.inner {
        if other.element.get_type().to_string() == i.element.get_type().to_string() {
          if i == &other {
            continue 'outer;
          }
          let old_type = other.element.get_type();
          other.element.set_type(format!(
            "{}{}",
            other.element.get_type().to_string(),
            to_struct_name(&format!("{:?}", other.name.ty))
          ));
          for implementation in &mut other.implementation {
            if implementation.target == old_type {
              implementation.target = other.element.get_type();
            }
          }
          break;
        }
      }
      self.inner.push(other);
    }
  }

  pub fn merge(&mut self, mut other: XsdImpl, settings: MergeSettings) {
    let children_are_attributes =
      matches!(other.name.ty, XsdType::Attribute | XsdType::AttributeGroup);

    let flatten_children =
      matches!(other.name.ty, XsdType::Group | XsdType::AttributeGroup) || other.flatten;

    match &mut self.element {
      XsdElement::Struct(a) => match &other.element {
        XsdElement::Struct(b) => match (&mut a.fields, &b.fields) {
          (Fields::Empty, b_fields) => {
            a.fields = b_fields.clone();
            self.inner.extend(other.inner);
          }
          (Fields::Tuple(a_fields), Fields::Tuple(b_fields)) => {
            for field in b_fields {
              let mut field = field.clone();
              field.attribute = children_are_attributes;
              field.flatten = flatten_children;
              a_fields.push(field);
            }
            self.merge_inner(other.inner);
          }
          (Fields::Named(a_fields), Fields::Named(b_fields)) => {
            for field in b_fields {
              let mut conflict = false;
              for name in &*a_fields {
                if name.name == field.name {
                  conflict = true;
                  break;
                }
              }

              if settings.conflict_prefix.is_none() {
                conflict = false;
              }

              let mut field = field.clone();
              field.attribute = children_are_attributes;
              field.flatten = flatten_children;

              if conflict {
                field.name = format!("{}{}", settings.conflict_prefix.unwrap(), field.name);
                a_fields.push(field);
              } else {
                a_fields.push(field);
              }
            }
            self.merge_inner(other.inner);
          }
          _ => {
            let field_name = to_field_name(
              other
                .fieldname_hint
                .as_ref()
                .unwrap_or_else(|| &b.ty().name),
            );
            let ty = b.ty().clone();

            other.fieldname_hint = Some(field_name.clone());
            let ty = ty.path(&to_field_name(&a.ty().name));

            let field = Field::new(
              ty.xml_name.clone(),
              &field_name,
              ty,
              children_are_attributes,
              flatten_children,
            )
            .vis("pub");
            a.push_field(field);

            self.merge_inner(vec![other]);
          }
        },
        XsdElement::Enum(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          let ty = ty.path(&to_field_name(&a.ty().name));

          let field = Field::new(
            ty.xml_name.clone(),
            &field_name,
            ty,
            children_are_attributes,
            flatten_children,
          )
          .vis("pub");
          a.push_field(field);

          self.merge_inner(vec![other]);
        }
        XsdElement::Type(b) | XsdElement::TypeAlias(b, _) => {
          let field_name = to_field_name(other.fieldname_hint.as_ref().unwrap_or(&b.name));

          let mut b = b.clone();
          for i in &mut other.inner {
            if let XsdElement::Field(_) | XsdElement::Type(_) | XsdElement::TypeAlias(_, _) =
              i.element
            {
              continue;
            }

            if i.element.get_type() == b {
              b = b.path(&to_field_name(&a.ty().to_string()));
            }

            let mut new_generics = vec![];
            for generic in b.generics {
              if i.element.get_type() == generic {
                new_generics.push(generic.path(&to_field_name(&a.ty().to_string())));
              } else {
                new_generics.push(generic);
              }
            }
            b.generics = new_generics;
          }

          let mut field = Field::new(
            b.xml_name.clone(),
            &field_name,
            b,
            children_are_attributes,
            flatten_children,
          )
          .vis("pub");

          let mut name_conflict = match &a.fields {
            Fields::Empty => false,
            Fields::Tuple(_) => false,
            Fields::Named(a_fields) => {
              let mut conflict = false;
              for a_field in a_fields {
                if a_field.name == field.name {
                  conflict = true;
                  break;
                }
              }

              conflict
            }
          };

          if settings.conflict_prefix.is_none() {
            name_conflict = false;
          }

          if name_conflict {
            field.name = format!("{}{}", settings.conflict_prefix.unwrap(), field.name);
          }

          a.push_field(field);

          self.merge_inner(other.inner);
        }
        XsdElement::Field(b) => match &mut a.fields {
          Fields::Empty => a.fields = Fields::Named(vec![b.clone()]),
          Fields::Tuple(fields) => {
            fields.push(TupleField {
              vis: b.vis.clone(),
              ty: b.ty.clone(),
              attribute: children_are_attributes,
              flatten: flatten_children,
            });
          }
          Fields::Named(fields) => {
            let mut conflict = false;
            for name in fields.iter() {
              if name.name == b.name {
                conflict = true;
                break;
              }
            }

            if settings.conflict_prefix.is_none() {
              conflict = false;
            }

            let mut field = b.clone();
            field.attribute = children_are_attributes;
            field.flatten = flatten_children;

            if conflict {
              field.name = format!("{}{}", settings.conflict_prefix.unwrap(), field.name);
            }
            fields.push(field);
          }
        },
      },
      XsdElement::Enum(a) => match &other.element {
        XsdElement::Struct(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          let ty = ty.path(&to_field_name(&a.ty().name));

          let variant = Variant::new(b.ty().xml_name.clone(), &field_name).tuple(
            ty,
            children_are_attributes,
            flatten_children,
          );
          a.variants.push(variant);

          self.merge_inner(vec![other]);
        }
        XsdElement::Enum(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let mut ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          ty.name = format!("{}::{}", to_field_name(&a.ty().name), ty.name);

          let variant = Variant::new(None, &to_struct_name(&field_name)).tuple(
            ty,
            children_are_attributes,
            flatten_children,
          );
          a.variants.push(variant);

          self.merge_inner(vec![other]);
        }
        XsdElement::Type(b) | XsdElement::TypeAlias(b, _) => {
          let field_name = to_struct_name(other.fieldname_hint.as_ref().unwrap_or(&b.name));

          let mut b = b.clone();
          for i in &mut other.inner {
            if let XsdElement::Field(_) | XsdElement::Type(_) | XsdElement::TypeAlias(_, _) =
              i.element
            {
              continue;
            }

            if i.element.get_type() == b {
              b = b.path(&to_field_name(&a.ty().to_string()));
            }

            let mut new_generics = vec![];
            for generic in b.generics {
              if i.element.get_type() == generic {
                new_generics.push(generic.path(&to_field_name(&a.ty().to_string())));
              } else {
                new_generics.push(generic);
              }
            }
            b.generics = new_generics;
          }

          let variant =
            Variant::new(None, &field_name).tuple(b, children_are_attributes, flatten_children);
          a.variants.push(variant);

          self.merge_inner(other.inner);
        }
        XsdElement::Field(b) => {
          let variant = Variant::new(None, &to_struct_name(&b.name)).tuple(
            b.ty.clone(),
            children_are_attributes,
            flatten_children,
          );
          a.variants.push(variant);
        }
      },
      XsdElement::Type(_) => unimplemented!("Cannot merge into type."),
      XsdElement::TypeAlias(..) => unimplemented!("Cannot merge into type alias."),
      XsdElement::Field(_) => unimplemented!("Cannot merge into field."),
    }
  }
}

pub enum SearchResult<'a> {
  MultipleMatches,
  NoMatches,
  SingleMatch(&'a XsdImpl),
}

#[derive(Clone, Debug)]
pub struct XsdContext {
  pub namespace: Namespace,
  pub xml_schema_prefix: Option<String>,
  pub structs: BTreeMap<XsdName, XsdImpl>,
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
            let namespace_uri = &name.namespace.unwrap();
            let impl_basic_type = |name: &str, ty: &str| -> (XsdName, XsdImpl) {
              let xsd_name = XsdName {
                namespace: Some(namespace_uri.clone()),
                local_name: name.to_string(),
                ty: XsdType::SimpleType,
              };

              // let mut r#impl = Impl::new(ty).impl_trait("ParseXsd").to_owned();
              // let func = r#impl.new_fn("parse");
              // func.line("element.get_content()?");
              // let func = r#impl.new_fn("parse_attribute");
              // func.line("element.get_attribute()?");

              let imp = XsdImpl {
                name: xsd_name.clone(),
                fieldname_hint: None,
                element: XsdElement::Type(Type::new(None, ty)),
                inner: vec![],
                implementation: vec![],
                flatten: false,
              };

              (xsd_name, imp)
            };

            return Ok(XsdContext {
              namespace,
              xml_schema_prefix: None,
              structs: BTreeMap::from_iter(
                [
                  ("bool", "bool"),
                  ("boolean", "bool"),
                  ("positiveInteger", "u64"),
                  ("byte", "u8"),
                  ("unsignedByte", "u8"),
                  ("short", "i16"),
                  ("unsignedShort", "u16"),
                  ("int", "i32"),
                  ("integer", "i32"),
                  ("unsignedInt", "u32"),
                  ("long", "i64"),
                  ("unsignedLong", "u64"),
                  ("nonNegativeInteger", "u64"),
                  ("double", "f64"),
                  ("decimal", "f64"),
                  ("string", "String"),
                  ("normalizedString", "String"),
                  ("anyURI", "String"),
                  ("NMTOKEN", "String"),
                  ("token", "String"),
                  ("language", "String"),
                  ("hexBinary", "String"),
                  ("dateTime", "String"),
                  ("base64Binary", "String"),
                  ("duration", "String"),
                  ("gYear", "u16"),
                  ("ID", "String"),
                  ("IDREF", "String"),
                  ("IDREFS", "String"),
                  ("anyType", "String"),
                  ("date", "Date"),
                  ("NCName", "String"),
                ]
                .map(|(n, t)| impl_basic_type(n, t)),
              ),
            });
          }
        }
        Err(_) => break,
        _ => {}
      }
    }

    Err(
      XsdIoError::XsdParseError(XsdParseError {
        node_name: "schema".to_string(),
        msg: "Bad XML Schema, unable to found schema element.".to_string(),
      })
      .into(),
    )
  }

  fn resolve_namespace(&self, namespace: Option<&str>) -> Option<String> {
    if let Some(ns) = namespace {
      if let Some(ns) = self.namespace.get(ns).map(|v| v.to_string()) {
        Some(ns)
      } else {
        namespace.map(|v| v.to_string())
      }
    } else {
      namespace.map(|v| v.to_string())
    }
  }

  pub fn remove_impl(&mut self, name: &XsdName) -> Option<XsdImpl> {
    let namespace = self.resolve_namespace(name.namespace.as_deref());

    self.structs.remove(&XsdName {
      namespace,
      local_name: name.local_name.clone(),
      ty: name.ty,
    })
  }

  pub fn insert_impl(&mut self, name: XsdName, mut value: XsdImpl) {
    let namespace = self.resolve_namespace(name.namespace.as_deref());

    let ty = value.element.get_type();

    for s in self.structs.values() {
      if s.element.get_type().to_string() == ty.to_string() {
        let old_type = value.element.get_type();
        value.element.set_type(format!(
          "{}{}",
          ty.to_string(),
          to_struct_name(&format!("{:?}", value.name.ty))
        ));
        for implementation in &mut value.implementation {
          if implementation.target == old_type {
            implementation.target = value.element.get_type();
          }
        }
        break;
      }
    }

    self.structs.insert(
      XsdName {
        namespace,
        local_name: name.local_name.clone(),
        ty: name.ty,
      },
      value,
    );
  }

  pub fn search(&self, name: &XsdName) -> Option<&XsdImpl> {
    let namespace = self.resolve_namespace(name.namespace.as_deref());

    self.structs.get(&XsdName {
      namespace,
      local_name: name.local_name.clone(),
      ty: name.ty,
    })
  }

  pub fn multi_search(
    &self,
    namespace: Option<String>,
    name: String,
    types: &[XsdType],
  ) -> SearchResult {
    let mut output = SearchResult::NoMatches;
    for ty in types {
      if let Some(result) = self.search(&XsdName {
        namespace: namespace.clone(),
        local_name: name.clone(),
        ty: *ty,
      }) {
        if let SearchResult::SingleMatch(_) = output {
          return SearchResult::MultipleMatches;
        }
        output = SearchResult::SingleMatch(result);
      }
    }

    output
  }
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
