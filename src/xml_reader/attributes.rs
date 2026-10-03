use std::iter::FromIterator;

use serde::ser::{SerializeMap, Serializer};

#[cfg(test)]
use crate::logiqx::RecordLocation;

use super::{AttributeLocation, AttributePosition, DeclaredText};

macro_rules! attribute_fields {
    ($name:ident { $($variant:ident = $code:literal => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(i64)]
        pub enum $name {
            $($variant = $code),+
        }

        impl $name {
            #[must_use]
            pub const fn code(self) -> i64 {
                self as i64
            }

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub(crate) fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($wire => Some(Self::$variant)),+,
                    _ => None,
                }
            }

            pub(crate) const fn from_code(code: i64) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant)),+,
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use attribute_fields;

/// Source-ordered XML attributes, each owning its value and declaration position.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XmlAttributes {
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name: String,
    declared: DeclaredText,
}

impl XmlAttributes {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&String> {
        self.get_declared(name).map(|declared| &declared.value)
    }

    #[must_use]
    pub fn get_declared(&self, name: &str) -> Option<&DeclaredText> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| &entry.declared)
    }

    #[must_use]
    pub fn contains_key(&self, name: &str) -> bool {
        self.get_declared(name).is_some()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|entry| &entry.name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.entries
            .iter()
            .map(|entry| (&entry.name, &entry.declared.value))
    }

    /// Iterate lexical attributes with their original position metadata.
    pub fn declared_iter(&self) -> impl Iterator<Item = (&str, &DeclaredText)> {
        self.entries
            .iter()
            .map(|entry| (entry.name.as_str(), &entry.declared))
    }

    /// Select closed native fields without copying their names or values.
    pub fn positions<Field>(
        &self,
        identify: impl Fn(&str) -> Option<Field>,
    ) -> impl Iterator<Item = AttributePosition<Field>> {
        self.declared_iter().filter_map(move |(name, declared)| {
            identify(name).map(|field| AttributePosition {
                field,
                source_order: declared.source_order,
                location: AttributeLocation {
                    line: declared.location.line,
                    column: declared.location.column,
                },
            })
        })
    }

    #[must_use]
    pub(crate) fn push_declared(&mut self, name: String, declared: DeclaredText) -> bool {
        if self.contains_key(&name) {
            return false;
        }
        self.entries.push(Entry { name, declared });
        true
    }

    pub(crate) fn update_value(&mut self, name: &str, value: String) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) else {
            return false;
        };
        entry.declared.value = value;
        true
    }

    fn insert_declared(&mut self, name: String, declared: DeclaredText) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) {
            entry.declared = declared;
        } else {
            self.entries.push(Entry { name, declared });
        }
    }
}

impl FromIterator<(String, DeclaredText)> for XmlAttributes {
    fn from_iter<T: IntoIterator<Item = (String, DeclaredText)>>(iter: T) -> Self {
        let mut attributes = Self::default();
        for (name, declared) in iter {
            attributes.insert_declared(name, declared);
        }
        attributes
    }
}

#[cfg(test)]
impl FromIterator<(String, String)> for XmlAttributes {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        let mut attributes = Self::default();
        for (source_order, (name, value)) in iter.into_iter().enumerate() {
            attributes.insert_declared(
                name,
                DeclaredText {
                    value,
                    source_order,
                    location: RecordLocation { line: 1, column: 1 },
                },
            );
        }
        attributes
    }
}

impl<'a> IntoIterator for &'a XmlAttributes {
    type Item = (&'a String, &'a String);
    type IntoIter = XmlAttributeIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        XmlAttributeIter {
            entries: self.entries.iter(),
        }
    }
}

/// Borrowed name/value pairs in original lexical order.
pub struct XmlAttributeIter<'a> {
    entries: std::slice::Iter<'a, Entry>,
}

impl<'a> Iterator for XmlAttributeIter<'a> {
    type Item = (&'a String, &'a String);

    fn next(&mut self) -> Option<Self::Item> {
        self.entries
            .next()
            .map(|entry| (&entry.name, &entry.declared.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl ExactSizeIterator for XmlAttributeIter<'_> {}
impl std::iter::FusedIterator for XmlAttributeIter<'_> {}

impl serde::Serialize for XmlAttributes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));

        let mut map = serializer.serialize_map(Some(entries.len()))?;
        for entry in entries {
            map.serialize_entry(&entry.name, &entry.declared.value)?;
        }
        map.end()
    }
}
