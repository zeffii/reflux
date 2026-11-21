//! The command table shared (in spirit) with Wax's animation instrument.
//!
//! A row in the animation lane is a list of `CC VVVV` pairs: a command byte and a
//! 16-bit value. Commands 00–0F are *modifiers*: they change how the next command
//! on the same row behaves and are then forgotten. Commands 10–7F are the fixed
//! standard set (never renumbered, only appended). Commands 80–FF are auxiliary:
//! each song declares its own, and Reflux decides what they mean.

use std::collections::BTreeMap;

/// Modifier: the next command glides to its value over this many ticks.
pub const GLIDE: u8 = 0x01;
/// Modifier: the next command uses this curve (see [`Curve::from_value`]).
pub const CURVE: u8 = 0x02;
/// Modifier: the next command adds a signed delta instead of setting a value.
pub const RELATIVE: u8 = 0x03;

pub fn is_modifier(command: u8) -> bool {
    command <= 0x0F
}

#[allow(dead_code)] // for the aux-command list once .wax parsing lands
pub fn is_auxiliary(command: u8) -> bool {
    command >= 0x80
}

/// The first command of the 16-wide group a command belongs to (0x29 -> 0x20).
/// Deputy nodes in the graph claim one of these groups.
pub fn group_of(command: u8) -> u8 {
    command & 0xF0
}

/// The shape of a glide from the old value to the new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Step,
    Linear,
    EaseIn,
    EaseOut,
    Smooth,
    Spring,
}

impl Curve {
    /// Decodes the value of a `02 VVVV` modifier.
    pub fn from_value(value: u16) -> Curve {
        match value {
            0 => Curve::Step,
            1 => Curve::Linear,
            2 => Curve::EaseIn,
            3 => Curve::EaseOut,
            4 => Curve::Smooth,
            5 => Curve::Spring,
            _ => Curve::Linear,
        }
    }

    /// Maps glide progress `p` (0..1) to blend amount. Spring overshoots and settles.
    pub fn shape(self, p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Curve::Step => {
                if p < 1.0 {
                    0.0
                } else {
                    1.0
                }
            }
            Curve::Linear => p,
            Curve::EaseIn => p * p * p,
            Curve::EaseOut => 1.0 - (1.0 - p).powi(3),
            Curve::Smooth => p * p * (3.0 - 2.0 * p),
            Curve::Spring => {
                if p >= 1.0 {
                    1.0
                } else {
                    1.0 - (-6.0 * p).exp() * (11.0 * p).cos()
                }
            }
        }
    }
}

/// How a command's events turn into a signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Changes how the next command on the row behaves.
    Modifier,
    /// Holds a value; new events glide towards their value.
    Target,
    /// Fires once; the signal is a decaying pulse scaled by the value.
    Trigger,
}

/// How the 16-bit value of a command is split into fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Whole16,
    /// `XXYY`: two independent bytes.
    Split8x8,
    /// `ABYY`: two nibbles and a byte.
    #[allow(dead_code)] // no demo command uses it yet
    Split4x4x8,
}

impl Layout {
    pub fn fields(self, value: u16) -> Vec<u16> {
        match self {
            Layout::Whole16 => vec![value],
            Layout::Split8x8 => vec![value >> 8, value & 0xFF],
            Layout::Split4x4x8 => vec![value >> 12, (value >> 8) & 0xF, value & 0xFF],
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // name is shown once deputies are built from the table
pub struct CommandInfo {
    pub name: String,
    pub kind: Kind,
    pub layout: Layout,
}

/// Every command Reflux knows for one song: the standard set plus that song's aux commands.
#[derive(Clone, Debug)]
pub struct CommandTable {
    commands: BTreeMap<u8, CommandInfo>,
}

impl CommandTable {
    /// The fixed standard set. Only the commands this prototype uses are listed so far;
    /// the numbering follows the agreed ranges (20 camera, 40 colour, 60 post fx, ...).
    pub fn standard() -> CommandTable {
        let mut table = CommandTable {
            commands: BTreeMap::new(),
        };
        let standard = [
            (GLIDE, "glide", Kind::Modifier),
            (CURVE, "curve", Kind::Modifier),
            (RELATIVE, "relative", Kind::Modifier),
            (0x10, "scene_cut", Kind::Trigger),
            (0x28, "fov", Kind::Target),
            (0x29, "orbit", Kind::Target),
            (0x2A, "radius", Kind::Target),
            (0x2B, "shake", Kind::Target),
            (0x41, "hue", Kind::Target),
            (0x44, "glow", Kind::Target),
            (0x60, "flash", Kind::Trigger),
        ];
        for (id, name, kind) in standard {
            table.insert(id, name, kind, Layout::Whole16);
        }
        table
    }

    /// Declares (or replaces) a command. Songs use this for their aux commands.
    pub fn insert(&mut self, id: u8, name: &str, kind: Kind, layout: Layout) {
        self.commands.insert(
            id,
            CommandInfo {
                name: name.to_string(),
                kind,
                layout,
            },
        );
    }

    pub fn get(&self, id: u8) -> Option<&CommandInfo> {
        self.commands.get(&id)
    }

    /// Unknown commands are treated as targets, so nothing in a song is silently dropped.
    pub fn kind_of(&self, id: u8) -> Kind {
        if is_modifier(id) {
            return Kind::Modifier;
        }
        self.get(id).map(|info| info.kind).unwrap_or(Kind::Target)
    }

    /// Splits a raw value into the fields its command's layout declares.
    pub fn fields_of(&self, id: u8, value: u16) -> Vec<u16> {
        self.get(id)
            .map(|info| info.layout)
            .unwrap_or(Layout::Whole16)
            .fields(value)
    }

    #[allow(dead_code)] // for node labels once deputies are built from the table
    pub fn name_of(&self, id: u8) -> String {
        self.get(id)
            .map(|info| info.name.clone())
            .unwrap_or_else(|| format!("cmd_{id:02X}"))
    }
}
