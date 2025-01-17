use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use std::convert::TryFrom;
use syn::spanned::Spanned;
use syn::{Attribute, Error, Lit, Meta, NestedMeta, Path, Result};

/// Values an [attr](self::Attr) can have
#[derive(Debug, Clone)]
pub enum Value {
    /// An identifier
    Ident(Ident),
    /// A literal value
    Lit(Lit),
}

impl ToTokens for Value {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Value::Ident(ident) => ident.to_tokens(tokens),
            Value::Lit(lit) => lit.to_tokens(tokens),
        }
    }
}

/// A simplified version of an attribute
#[derive(Debug, Clone)]
pub struct Attr {
    /// The path of this attribute
    ///
    /// e.g.: In `#[name = "some"]`, the part of `name` is the path of the attribute
    pub path: Path,
    /// The data type the attribute can have
    pub values: Vec<Value>,
}

impl Attr {
    /// Creates a new [attr](self::Attr)
    pub fn new(path: Path, values: Vec<Value>) -> Self {
        Self { path, values }
    }
}

impl Attr {
    #[allow(dead_code)]
    /// Executes the given function into the [attr](self::Attr)
    pub fn parse_value<T>(&self, f: impl FnOnce(&Value) -> Result<T>) -> Result<T> {
        if self.values.is_empty() {
            return Err(Error::new(self.span(), "Attribute input must not be empty"));
        }

        if self.values.len() > 1 {
            return Err(Error::new(
                self.span(),
                "Attribute input must not exceed more than one argument",
            ));
        }

        f(&self.values[0])
    }

    #[allow(dead_code)]
    pub fn parse_all(&self) -> Result<Vec<Ident>> {
        self.values
            .iter()
            .map(|v| match v {
                Value::Ident(ident) => Ok(ident.clone()),
                Value::Lit(Lit::Str(inner)) => {
                    Ok(Ident::new(inner.value().as_str(), Span::call_site()))
                }
                other => Err(Error::new(other.span(), "Not supported")),
            })
            .collect::<Result<_>>()
    }

    #[allow(dead_code)]
    /// Gets all the identifiers this attribute has, returning an error if this attribute has literal
    /// values instead of identifiers
    pub fn parse_identifiers(&self) -> Result<Vec<Ident>> {
        self.values
            .iter()
            .map(|v| match v {
                Value::Ident(ident) => Ok(ident.clone()),
                Value::Lit(lit) => Err(Error::new(lit.span(), "Literals are forbidden")),
            })
            .collect::<Result<Vec<_>>>()
    }

    #[allow(dead_code)]
    /// Parses the first identifier this attribute has, returning an error if this attribute does not
    /// have any of them or has literal values instead of identifiers
    pub fn parse_identifier(&self) -> Result<Ident> {
        self.parse_value(|value| {
            Ok(match value {
                Value::Ident(ident) => ident.clone(),
                _ => return Err(Error::new(value.span(), "Argument must be an identifier")),
            })
        })
    }

    #[allow(dead_code)]
    /// Parses the first literal into a string, returning an error if this attribute does not have any
    /// of them or has identifiers instead of literals
    pub fn parse_string(&self) -> Result<String> {
        self.parse_value(|value| {
            Ok(match value {
                Value::Lit(Lit::Str(s)) => s.value(),
                _ => return Err(Error::new(value.span(), "Argument must be a string")),
            })
        })
    }

    #[allow(dead_code)]
    /// Parses the first literal into a bool, returning an error if this attribute does not have any
    /// of them or has identifiers instead of literals
    pub fn parse_bool(&self) -> Result<bool> {
        self.parse_value(|value| {
            Ok(match value {
                Value::Lit(Lit::Bool(b)) => b.value,
                _ => return Err(Error::new(value.span(), "Argument must be a boolean")),
            })
        })
    }
}

impl ToTokens for Attr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Attr { path, values } = self;

        tokens.extend(if values.is_empty() {
            quote::quote!(#[#path])
        } else {
            quote::quote!(#[#path(#(#values)*,)])
        });
    }
}

impl TryFrom<&Attribute> for Attr {
    type Error = Error;

    fn try_from(attr: &Attribute) -> Result<Self> {
        parse_attribute(attr)
    }
}

/// Parses the given syn attribute into an attr
pub fn parse_attribute(attr: &Attribute) -> Result<Attr> {
    let meta = attr.parse_meta()?;

    match meta {
        Meta::Path(p) => Ok(Attr::new(p, Vec::new())),
        Meta::List(l) => {
            let path = l.path;
            let values = l
                .nested
                .into_iter()
                .map(|m| match m {
                    NestedMeta::Lit(lit) => Ok(Value::Lit(lit)),
                    NestedMeta::Meta(m) => match m {
                        Meta::Path(p) => Ok(Value::Ident(p.get_ident().unwrap().clone())),
                        _ => Err(Error::new(
                            m.span(),
                            "Nested lists or name values are not supported",
                        )),
                    },
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(Attr::new(path, values))
        }
        Meta::NameValue(nv) => Ok(Attr::new(nv.path, vec![Value::Lit(nv.lit)])),
    }
}
