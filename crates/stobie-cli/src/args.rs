//! Command-line parsing against the command registry.

use std::collections::BTreeMap;

use stobie_client::ClientError;
use stobie_client::protocol::{DomainReference, InspectionSelector, SemanticUiId};

use crate::spec::{self, Command};

/// One parsed command line.
#[derive(Debug, Default)]
pub struct Args {
    pub positional: Vec<String>,
    pub flags: BTreeMap<&'static str, Option<String>>,
    /// Everything after `--`.
    pub rest: Vec<String>,
}

impl Args {
    /// Parse `arguments` (after the command name) for `command`, rejecting
    /// unknown flags and missing values.
    pub fn parse(command: &Command, arguments: &[String]) -> Result<Self, ClientError> {
        let mut known: Vec<&spec::Flag> = command.flags.iter().collect();
        if command.connects {
            known.extend(spec::TARGET_FLAGS);
            known.extend(spec::SELECTOR_FLAGS);
        }
        let mut parsed = Self::default();
        let mut iterator = arguments.iter();
        while let Some(argument) = iterator.next() {
            if argument == "--" {
                parsed.rest = iterator.cloned().collect();
                break;
            }
            if argument.starts_with("--") && argument.len() > 2 {
                let (name, inline) = match argument.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_owned())),
                    None => (argument.as_str(), None),
                };
                let Some(flag) = known.iter().find(|flag| flag.name == name) else {
                    return Err(ClientError::usage(format!(
                        "`{}` does not accept {name}; see `stobie {} --help`",
                        command.name, command.name
                    )));
                };
                let value = match (flag.value, inline) {
                    (None, None) => None,
                    (None, Some(_)) => {
                        return Err(ClientError::usage(format!("{name} takes no value")));
                    }
                    (Some(_), Some(value)) => Some(value),
                    (Some(placeholder), None) => {
                        Some(iterator.next().cloned().ok_or_else(|| {
                            ClientError::usage(format!(
                                "{name} needs a value: {name} {placeholder}"
                            ))
                        })?)
                    }
                };
                if parsed.flags.insert(flag.name, value).is_some() {
                    return Err(ClientError::usage(format!("{name} is given twice")));
                }
            } else {
                parsed.positional.push(argument.clone());
            }
        }
        Ok(parsed)
    }

    pub fn has(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(|value| value.as_deref())
    }

    pub fn number<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, ClientError> {
        self.value(name)
            .map(|value| {
                value.parse().map_err(|_| {
                    ClientError::usage(format!("{name} needs a number, not {value:?}"))
                })
            })
            .transpose()
    }

    /// The selector from `--selector` and the individual selector flags, with
    /// `positional` (if any) as the given field.
    pub fn selector(
        &self,
        positional: Option<(&str, &str)>,
    ) -> Result<InspectionSelector, ClientError> {
        let mut selector = match self.value("--selector") {
            Some(json) => serde_json::from_str::<InspectionSelector>(json).map_err(|error| {
                ClientError::usage(format!(
                    "--selector is not a selector: {error}; fields are id, role, name, capability, pane, domain and argument"
                ))
            })?,
            None => InspectionSelector::default(),
        };
        let mut set = |field: &str, value: &str| -> Result<(), ClientError> {
            match field {
                "id" => selector.id = Some(SemanticUiId::new(value)),
                "capability" => selector.capability = Some(value.to_owned()),
                "name" => selector.name = Some(value.to_owned()),
                "role" => {
                    selector.role = Some(
                        serde_json::from_value(serde_json::Value::String(value.to_owned()))
                            .map_err(|_| {
                                ClientError::usage(format!(
                                    "unknown role {value:?}; roles are snake_case, such as button, text_input, combo_box, tab or result_row"
                                ))
                            })?,
                    )
                }
                "pane" => {
                    selector.pane = Some(
                        serde_json::from_value(serde_json::json!(value.parse::<u64>().map_err(
                            |_| ClientError::usage(format!("--pane needs a number, not {value:?}"))
                        )?))
                        .map_err(|error| ClientError::usage(format!("--pane: {error}")))?,
                    )
                }
                "domain" => {
                    selector.domain = Some(serde_json::from_str::<DomainReference>(value).map_err(
                        |error| {
                            ClientError::usage(format!(
                                "--domain is not a domain reference: {error}; for example {{\"kind\":\"pane\",\"value\":1}}"
                            ))
                        },
                    )?)
                }
                // A JSON number, boolean or string; anything else is text.
                "argument" => {
                    selector.argument = Some(
                        serde_json::from_str::<serde_json::Value>(value)
                            .ok()
                            .filter(|value| {
                                value.is_number() || value.is_boolean() || value.is_string()
                            })
                            .unwrap_or_else(|| serde_json::Value::String(value.to_owned())),
                    )
                }
                _ => unreachable!("known selector field"),
            }
            Ok(())
        };
        if let Some((field, value)) = positional {
            set(field, value)?;
        }
        for field in [
            "id",
            "role",
            "name",
            "capability",
            "pane",
            "domain",
            "argument",
        ] {
            if let Some(value) = self.value(&format!("--{field}")) {
                set(field, value)?;
            }
        }
        Ok(selector)
    }
}
