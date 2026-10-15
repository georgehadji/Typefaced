//! A designspace 5 document, mirroring norad 0.18.4's `designspace` types field for field.
//!
//! M1 loads and saves designspaces but does not edit them, so these are plain values. Numbers
//! stay `f32`, as norad reads them, so a round trip is exact. A `<source>` element is a
//! [`Master`]: it points at a model [`Source`](crate::Source) (one UFO) and optionally one of
//! its layers, because several masters may share a UFO (sparse masters).

use crate::{LayerId, LibDict, Name, SourceId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Designspace {
    /// The `format` attribute (`5.0`, `4.1`, …).
    pub format: f32,
    pub axes: Vec<Axis>,
    pub axis_mappings: Option<AxisMappings>,
    pub rules: Rules,
    pub masters: Vec<Master>,
    pub instances: Vec<Instance>,
    pub lib: LibDict,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub name: String,
    pub tag: String,
    pub default: f32,
    pub hidden: bool,
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,
    /// Discrete axis values.
    pub values: Option<Vec<f32>>,
    pub map: Option<Vec<AxisMapping>>,
    pub label_names: Vec<LocalizedString>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalizedString {
    pub language: String,
    pub string: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisMapping {
    pub input: f32,
    pub output: f32,
}

/// Designspace 5.1 `<mappings>` (avar2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxisMappings {
    pub description: Option<String>,
    pub mappings: Vec<AxisMappingEntry>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxisMappingEntry {
    pub description: Option<String>,
    pub input: Vec<Dimension>,
    pub output: Vec<Dimension>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rules {
    pub processing: RuleProcessing,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RuleProcessing {
    #[default]
    First,
    Last,
}

/// norad folds legacy bare `<condition>` elements into one condition set on load.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule {
    pub name: Option<String>,
    pub condition_sets: Vec<ConditionSet>,
    pub substitutions: Vec<Substitution>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Substitution {
    pub name: Name,
    pub with: Name,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConditionSet {
    pub conditions: Vec<Condition>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Condition {
    pub name: String,
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,
}

/// A `<source>` element.
#[derive(Clone, Debug, PartialEq)]
pub struct Master {
    pub name: Option<String>,
    pub familyname: Option<String>,
    pub stylename: Option<String>,
    /// The `filename` attribute, verbatim (path of the UFO relative to the designspace).
    pub filename: String,
    /// The UFO it loads.
    pub source: SourceId,
    /// The `layer` attribute resolved to a layer of `source`; `None` when the attribute is
    /// absent (the UFO's default layer).
    pub layer: Option<LayerId>,
    pub location: Vec<Dimension>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instance {
    pub name: Option<String>,
    pub familyname: Option<String>,
    pub stylename: Option<String>,
    pub filename: Option<String>,
    pub postscriptfontname: Option<String>,
    pub stylemapfamilyname: Option<String>,
    pub stylemapstylename: Option<String>,
    pub location: Vec<Dimension>,
    pub lib: LibDict,
}

/// One axis coordinate of a location.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dimension {
    pub name: String,
    pub uservalue: Option<f32>,
    pub xvalue: Option<f32>,
    pub yvalue: Option<f32>,
}
