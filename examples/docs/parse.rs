//! A small reader of the crate's own source: public items with their `///` docs,
//! enough to lay a reference page out from. It leans on the code being rustfmt'd
//! (items at column 0, methods at column 4, one signature per line or one argument
//! per line) rather than parsing Rust.

use std::fs;
use std::path::Path;

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Fn,
    Struct,
    Enum,
    Type,
    Const,
    Trait,
}

/// A documented variant of an enum, or field of a struct.
#[derive(Clone, Debug)]
pub struct Member {
    pub name: String,
    /// The declaration line, without its trailing punctuation.
    pub sig: String,
    pub docs: String,
    /// Nesting: `0` for a variant or field, `1` for a field of a struct variant.
    pub depth: usize,
}

/// One public item.
#[derive(Clone, Debug)]
pub struct Item {
    pub kind: Kind,
    /// The type a method belongs to.
    pub owner: Option<String>,
    pub name: String,
    pub sig: String,
    pub docs: String,
    pub members: Vec<Member>,
}

impl Item {
    /// How the item is referred to: `Canvas::fill_rect`, `Paint`.
    pub fn key(&self) -> String {
        match &self.owner {
            Some(o) => format!("{o}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// One source file's worth of documentation.
#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    pub docs: String,
    pub items: Vec<Item>,
}

/// Reads `path` as module `name`.
pub fn module(name: &str, path: impl AsRef<Path>) -> Module {
    let src = fs::read_to_string(path.as_ref()).unwrap_or_else(|e| panic!("{}: {e}", path.as_ref().display()));
    let lines: Vec<&str> = src.lines().collect();
    let docs = lines.iter().take_while(|l| l.starts_with("//!") || l.is_empty()).filter_map(|l| doc_text(l, "//!"));
    let docs = docs.collect::<Vec<_>>().join("\n");

    let mut items = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut owner: Option<String> = None;
    // Inside a trait impl, `impl Trait for T`, whose methods are not listed.
    let mut skipping = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        if let Some(text) = doc_text(trimmed, "///") {
            pending.push(text.to_string());
            i += 1;
            continue;
        }
        if trimmed.starts_with("#[") {
            i += 1;
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            if line == "}" {
                owner = None;
                skipping = false;
            } else if let Some(rest) = line.strip_prefix("impl") {
                skipping = rest.contains(" for ");
                owner = (!skipping).then(|| impl_type(rest));
            } else if let Some((kind, name)) = item_start(line) {
                let (sig, end) = signature(&lines, i);
                let mut item = Item {
                    kind,
                    owner: None,
                    name,
                    sig,
                    docs: std::mem::take(&mut pending).join("\n"),
                    members: Vec::new(),
                };
                if matches!(kind, Kind::Struct | Kind::Enum) && lines[end].trim_end().ends_with('{') {
                    let (members, close) = members(&lines, end + 1, kind);
                    item.members = members;
                    i = close;
                }
                items.push(item);
                i = i.max(end);
            }
        } else if indent == 4
            && owner.is_some()
            && !skipping
            && let Some((kind, name)) = item_start(trimmed)
        {
            let (sig, end) = signature(&lines, i);
            items.push(Item {
                kind,
                owner: owner.clone(),
                name,
                sig,
                docs: std::mem::take(&mut pending).join("\n"),
                members: Vec::new(),
            });
            i = end;
        }
        pending.clear();
        i += 1;
    }
    Module { name: name.to_string(), docs, items }
}

/// The text of a doc comment line, `None` for anything else.
fn doc_text<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(marker)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// The type an `impl` header is for: `impl<'a> Bubble<'a> {` is `Bubble`.
fn impl_type(rest: &str) -> String {
    let mut s = rest.trim_start();
    if s.starts_with('<') {
        let mut depth = 0;
        for (k, ch) in s.char_indices() {
            depth += (ch == '<') as i32 - (ch == '>') as i32;
            if depth == 0 {
                s = &s[k + 1..];
                break;
            }
        }
    }
    s.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}

/// Whether a line starts a public item, and its kind and name.
fn item_start(line: &str) -> Option<(Kind, String)> {
    let rest = line.strip_prefix("pub ")?;
    let (kind, rest) = if let Some(r) = rest.strip_prefix("fn ") {
        (Kind::Fn, r)
    } else if let Some(r) = rest.strip_prefix("const fn ") {
        (Kind::Fn, r)
    } else if let Some(r) = rest.strip_prefix("struct ") {
        (Kind::Struct, r)
    } else if let Some(r) = rest.strip_prefix("enum ") {
        (Kind::Enum, r)
    } else if let Some(r) = rest.strip_prefix("type ") {
        (Kind::Type, r)
    } else if let Some(r) = rest.strip_prefix("const ") {
        (Kind::Const, r)
    } else {
        (Kind::Trait, rest.strip_prefix("trait ")?)
    };
    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some((kind, name))
}

/// The signature starting at `lines[i]`, joined onto one line without its body,
/// and the index of its last line.
fn signature(lines: &[&str], i: usize) -> (String, usize) {
    let mut parts = Vec::new();
    let mut end = i;
    for (k, line) in lines.iter().enumerate().skip(i) {
        let t = line.trim();
        parts.push(t.to_string());
        end = k;
        if t.ends_with('{') || t.ends_with(';') || t.ends_with('}') {
            break;
        }
    }
    let mut sig = parts.join(" ");
    for (from, to) in [("( ", "("), (", )", ")"), (" {", ""), ("{", ""), (";", "")] {
        sig = sig.replace(from, to);
    }
    // A one-line `pub const X: T = value;` keeps its value; a struct's `{` is gone.
    (sig.trim_end().to_string(), end)
}

/// The documented members of the struct or enum whose body starts at `lines[i]`,
/// and the index of its closing brace.
fn members(lines: &[&str], i: usize, kind: Kind) -> (Vec<Member>, usize) {
    let mut out = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut k = i;
    while k < lines.len() {
        let line = lines[k];
        if line == "}" {
            break;
        }
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(text) = doc_text(trimmed, "///") {
            pending.push(text.to_string());
        } else if trimmed.starts_with("#[") || trimmed.is_empty() || trimmed == "}," || trimmed == "}" {
            // Attributes keep their docs; a closing brace of a struct variant ends it.
        } else {
            let depth = (indent / 4).saturating_sub(1);
            let public = kind == Kind::Enum || depth == 1 || trimmed.starts_with("pub ");
            let decl = trimmed.trim_end_matches([',', '{', ' ']);
            let name: String =
                decl.trim_start_matches("pub ").chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if public && !name.is_empty() {
                out.push(Member { name, sig: decl.to_string(), docs: pending.join("\n"), depth });
            }
            pending.clear();
        }
        k += 1;
    }
    (out, k)
}
