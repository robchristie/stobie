//! Client errors: the application's protocol errors, plus what the client
//! itself establishes (transport, deadlines, uncertain completion).

use serde::Serialize;
use stobie_protocol::{CompletedObservation, InspectionError, InspectionErrorCode};

/// Why a client operation failed. Codes are stable snake_case strings: every
/// protocol [`InspectionErrorCode`] plus the client's own codes below.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClientError {
    pub code: String,
    pub message: String,
    /// What to do next.
    pub hint: String,
    /// The mutation whose outcome is unknown, for `uncertain_completion`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// The latest observation a wait saw before it gave up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<Box<CompletedObservation>>,
    /// The protocol error behind a client error, when there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<InspectionError>>,
}

/// The class of a failure, which the CLI maps to its exit status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// The application rejected or could not match the request.
    Application,
    /// The request itself was malformed.
    Usage,
    /// No application could be reached, or it is another instance.
    Connection,
    /// A deadline passed or a mutation's outcome is unknown.
    Deadline,
}

impl ClientError {
    pub fn new(code: &str, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint: hint.into(),
            request_id: None,
            observation: None,
            cause: None,
        }
    }

    /// The application's protocol error, with advice for its code.
    pub fn protocol(error: InspectionError) -> Self {
        let code = serde_json::to_value(error.code)
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "protocol_error".into());
        let hint = match error.code {
            InspectionErrorCode::NoMatch => {
                "observe or discover to find current IDs and capabilities"
            }
            InspectionErrorCode::Ambiguous => "add id, pane or domain to the selector",
            InspectionErrorCode::StaleTarget => {
                "the target changed since it was discovered; discover it again"
            }
            InspectionErrorCode::Unavailable => "the control is disabled or hidden; see the reason",
            InspectionErrorCode::NotInvocable => {
                "this capability needs physical input, such as a click"
            }
            InspectionErrorCode::InvalidArguments => {
                "discover the capability to see its argument contract"
            }
            InspectionErrorCode::WrongInstance => "the application restarted; attach again",
            InspectionErrorCode::MissingObservation => {
                "the application has not completed a UI pass yet; retry shortly"
            }
            InspectionErrorCode::StaleCursor => "start the query again without a cursor",
            InspectionErrorCode::QueueFull => "wait for queued invocations to finish",
            InspectionErrorCode::HistoryFull => {
                "this instance admits no more mutations; restart the application"
            }
            InspectionErrorCode::TooLate => "the action already started or finished",
            InspectionErrorCode::UnknownRequest => "no mutation used this request ID",
            InspectionErrorCode::ValidationFailed => "the application refused it; see the message",
            InspectionErrorCode::RequestIdConflict => {
                "this request ID was already used: read `stobie receipt ID` and do not repeat the action"
            }
            InspectionErrorCode::UnsupportedVersion
            | InspectionErrorCode::UnsupportedOperation
            | InspectionErrorCode::InvalidRequest
            | InspectionErrorCode::PublicationLimit => "check the request against `capabilities`",
        };
        Self {
            code,
            message: error.message.clone(),
            hint: hint.into(),
            request_id: None,
            observation: None,
            cause: Some(Box::new(error)),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(
            "usage",
            message,
            "see `stobie --help` and `stobie capabilities --json`",
        )
    }

    pub fn connection(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::new("connection_failed", message, hint)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(
            "timeout",
            message,
            "raise --timeout, or check that the application is visible and responsive",
        )
    }

    /// A mutation was sent but its outcome is unknown. Never repeat it; read
    /// its receipt instead.
    pub fn uncertain(request_id: &str, message: impl Into<String>, cause: Option<Self>) -> Self {
        Self {
            code: "uncertain_completion".into(),
            message: message.into(),
            hint: format!("do not repeat the action; run `stobie receipt {request_id}`"),
            request_id: Some(request_id.into()),
            observation: cause.as_ref().and_then(|cause| cause.observation.clone()),
            cause: cause.and_then(|cause| cause.cause),
        }
    }

    pub fn with_observation(mut self, observation: CompletedObservation) -> Self {
        self.observation = Some(Box::new(observation));
        self
    }

    pub fn class(&self) -> ErrorClass {
        match self.code.as_str() {
            "usage" | "invalid_request" | "unsupported_version" | "unsupported_operation" => {
                ErrorClass::Usage
            }
            "connection_failed" | "wrong_instance" | "protocol_violation" => ErrorClass::Connection,
            "timeout" | "uncertain_completion" => ErrorClass::Deadline,
            _ => ErrorClass::Application,
        }
    }
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ClientError {}
