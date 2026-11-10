use std::collections::BTreeMap;
use std::fmt;

/// A Bedrock block-state value. Bedrock stores booleans as bytes, ints as ints
/// and everything else as strings, so the importer maps onto these three.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateValue {
    Bool(bool),
    Byte(i8),
    Int(i32),
    Str(String),
}

impl StateValue {
    pub fn parse(s: &str) -> Self {
        match s {
            "true" => StateValue::Bool(true),
            "false" => StateValue::Bool(false),
            _ => s
                .parse::<i32>()
                .map(StateValue::Int)
                .unwrap_or_else(|_| StateValue::Str(s.to_string())),
        }
    }
}

impl fmt::Display for StateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateValue::Bool(b) => write!(f, "{b}"),
            StateValue::Byte(i) => write!(f, "{i}"),
            StateValue::Int(i) => write!(f, "{i}"),
            StateValue::Str(s) => write!(f, "{s}"),
        }
    }
}

/// Namespaced block identifier plus its states, e.g.
/// `minecraft:oak_stairs[weirdo_direction=2,upside_down_bit=false]`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState {
    pub name: String,
    pub states: BTreeMap<String, StateValue>,
}

impl BlockState {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            states: BTreeMap::new(),
        }
    }

    pub fn air() -> Self {
        Self::new("minecraft:air")
    }

    pub fn with(mut self, key: &str, value: StateValue) -> Self {
        self.states.insert(key.to_string(), value);
        self
    }

    pub fn is_air(&self) -> bool {
        matches!(
            self.name.as_str(),
            "minecraft:air"
                | "minecraft:cave_air"
                | "minecraft:void_air"
                | "minecraft:structure_void"
        )
    }

    /// Parses `name` or `name[key=value,key=value]`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let Some(open) = s.find('[') else {
            return if s.is_empty() {
                None
            } else {
                Some(Self::new(s))
            };
        };
        let body = s[open + 1..].strip_suffix(']')?;
        let mut out = Self::new(&s[..open]);
        if out.name.is_empty() {
            return None;
        }
        for entry in body.split(',').filter(|e| !e.trim().is_empty()) {
            let (k, v) = entry.split_once('=')?;
            out.states
                .insert(k.trim().to_string(), StateValue::parse(v.trim()));
        }
        Some(out)
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.states.is_empty() {
            write!(f, "[")?;
            for (i, (k, v)) in self.states.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{k}={v}")?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}
