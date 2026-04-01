//! # nc-ir — Typed Intermediate Representation
//!
//! The whitelist layer that constrains AI output. An LLM's S-expression must
//! successfully lower into these Rust enums — or it is rejected outright.
//!
//! ## Design Principle
//!
//! The AI cannot invent a new HTTP method, input type, or auth mode.
//! It can only select from the variants defined here. This is the "GPU shader
//! instruction set" — finite, verified, deterministic.

use nc_parser::SexpNode;
use std::fmt;

// ---------------------------------------------------------------------------
// IR types — the whitelist
// ---------------------------------------------------------------------------

/// HTTP method. Only these four are legal.
#[derive(Debug, Clone, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// Input specification — how the endpoint receives data.
#[derive(Debug, Clone, PartialEq)]
pub enum InputSpec {
    /// JSON body with a named schema type.
    Json { schema: String },
    /// Multipart file upload.
    File { max_size: Option<String>, types: Vec<String> },
    /// URL query parameters.
    Query { params: Vec<String> },
    /// No input.
    None,
}

/// Output specification — what the endpoint returns.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputSpec {
    /// JSON response with a named schema type.
    Json { schema: String },
    /// Plain text response.
    Text,
    /// No content (204).
    NoContent,
}

/// Authentication requirement.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthRequirement {
    Required,
    Optional,
    None,
}

/// Rate limit specification.
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimit {
    pub count: u32,
    pub period: String,
}

/// State dependency — what shared resources the handler needs.
/// The AI writes `(state db-pool)` — the generator handles Arc, Mutex, Clone, with_state.
#[derive(Debug, Clone, PartialEq)]
pub enum StateDep {
    /// Database connection pool (generates Arc<Mutex<DbPool>>).
    DbPool,
    /// In-memory cache (generates Arc<Mutex<HashMap>>).
    Cache,
}

/// A fully validated API endpoint specification.
/// Every field is typed and constrained — no arbitrary strings, no hallucination.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiEndpoint {
    pub method: HttpMethod,
    pub path: String,
    pub input: InputSpec,
    pub output: OutputSpec,
    pub auth: AuthRequirement,
    pub rate_limit: Option<RateLimit>,
    pub state: Vec<StateDep>,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Validation errors — structured, not compiler stack traces
// ---------------------------------------------------------------------------

/// Structured error returned when an S-expression fails IR validation.
/// These errors are designed to be fed back to the LLM for self-correction.
#[derive(Debug, Clone)]
pub struct IrError {
    pub field: String,
    pub message: String,
    pub expected: Vec<String>,
    pub got: String,
}

impl fmt::Display for IrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IR validation error in `{}`: {} (expected one of {:?}, got {:?})",
            self.field, self.message, self.expected, self.got)
    }
}

impl std::error::Error for IrError {}

// ---------------------------------------------------------------------------
// S-expr → IR lowering
// ---------------------------------------------------------------------------

impl ApiEndpoint {
    /// Lower a parsed S-expression into a validated `ApiEndpoint`.
    ///
    /// Input format:
    /// ```lisp
    /// (api :method POST
    ///      :path "/users/me/avatar"
    ///      :input (file :max-size "5MB" :types ("image/png" "image/jpeg"))
    ///      :output (json :schema UserAvatar)
    ///      :auth required
    ///      :rate-limit "10/min"
    ///      :description "Upload user avatar image")
    /// ```
    ///
    /// Returns `Err(IrError)` if any field is missing, has an invalid value,
    /// or uses a type not in the whitelist.
    pub fn from_sexp(node: &SexpNode) -> Result<Self, IrError> {
        // Tag must be "api"
        let tag = node.tag().ok_or_else(|| IrError {
            field: "tag".into(),
            message: "expected a list with tag".into(),
            expected: vec!["api".into()],
            got: format!("{node}"),
        })?;
        if tag != "api" {
            return Err(IrError {
                field: "tag".into(),
                message: "wrong tag".into(),
                expected: vec!["api".into()],
                got: tag.into(),
            });
        }

        // :method (required)
        let method_str = node.keyword(":method").ok_or_else(|| IrError {
            field: ":method".into(),
            message: "missing required field :method".into(),
            expected: vec!["GET".into(), "POST".into(), "PUT".into(), "DELETE".into()],
            got: String::new(),
        })?;
        let method = match method_str.to_uppercase().as_str() {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            other => return Err(IrError {
                field: ":method".into(),
                message: "invalid HTTP method".into(),
                expected: vec!["GET".into(), "POST".into(), "PUT".into(), "DELETE".into()],
                got: other.into(),
            }),
        };

        // :path (required)
        let path = node.keyword(":path").ok_or_else(|| IrError {
            field: ":path".into(),
            message: "missing required field :path".into(),
            expected: vec!["\"/your/endpoint\"".into()],
            got: String::new(),
        })?.to_string();

        if !path.starts_with('/') {
            return Err(IrError {
                field: ":path".into(),
                message: "path must start with '/'".into(),
                expected: vec!["\"/your/endpoint\"".into()],
                got: path,
            });
        }

        // :input (optional, defaults to None)
        let input = match node.keyword_node(":input") {
            Some(input_node) => parse_input_spec(input_node)?,
            None => match node.keyword(":input") {
                Some("none") | None => InputSpec::None,
                Some(other) => return Err(IrError {
                    field: ":input".into(),
                    message: "invalid input spec".into(),
                    expected: vec!["(json :schema T)".into(), "(file ...)".into(), "(query ...)".into(), "none".into()],
                    got: other.into(),
                }),
            },
        };

        // :output (optional, defaults to NoContent)
        let output = match node.keyword_node(":output") {
            Some(output_node) => parse_output_spec(output_node)?,
            None => match node.keyword(":output") {
                Some("text") => OutputSpec::Text,
                Some("none") | None => OutputSpec::NoContent,
                Some(other) => return Err(IrError {
                    field: ":output".into(),
                    message: "invalid output spec".into(),
                    expected: vec!["(json :schema T)".into(), "text".into(), "none".into()],
                    got: other.into(),
                }),
            },
        };

        // :auth (optional, defaults to None)
        let auth = match node.keyword(":auth") {
            Some("required") => AuthRequirement::Required,
            Some("optional") => AuthRequirement::Optional,
            Some("none") | None => AuthRequirement::None,
            Some(other) => return Err(IrError {
                field: ":auth".into(),
                message: "invalid auth requirement".into(),
                expected: vec!["required".into(), "optional".into(), "none".into()],
                got: other.into(),
            }),
        };

        // :rate-limit (optional)
        let rate_limit = match node.keyword(":rate-limit") {
            Some(spec) => Some(parse_rate_limit(spec)?),
            None => None,
        };

        // :state (optional) — e.g. `:state (db-pool cache)` or `:state (db-pool)`
        let state = match node.keyword_node(":state") {
            Some(state_node) => parse_state_deps(state_node)?,
            None => match node.keyword(":state") {
                Some(single) => parse_single_state_dep(single).map(|d| vec![d])?,
                None => vec![],
            },
        };

        // :description (optional)
        let description = node.keyword(":description").map(String::from);

        Ok(ApiEndpoint { method, path, input, output, auth, rate_limit, state, description })
    }
}

fn parse_input_spec(node: &SexpNode) -> Result<InputSpec, IrError> {
    let tag = node.tag().unwrap_or("");
    match tag {
        "json" => {
            let schema = node.keyword(":schema").ok_or_else(|| IrError {
                field: ":input (json)".into(),
                message: "json input requires :schema".into(),
                expected: vec!["(json :schema TypeName)".into()],
                got: format!("{node}"),
            })?;
            Ok(InputSpec::Json { schema: schema.into() })
        }
        "file" => {
            let max_size = node.keyword(":max-size").map(String::from);
            let types = match node.keyword_node(":types") {
                Some(SexpNode::List(items)) => {
                    items.iter().skip(1).filter_map(|i| {
                        if let SexpNode::Atom(s) = i { Some(s.clone()) } else { None }
                    }).collect()
                }
                _ => vec![],
            };
            Ok(InputSpec::File { max_size, types })
        }
        "query" => {
            let params = match node {
                SexpNode::List(items) => {
                    items.iter().skip(1).filter_map(|i| {
                        if let SexpNode::Atom(s) = i { if !s.starts_with(':') { Some(s.clone()) } else { None } } else { None }
                    }).collect()
                }
                _ => vec![],
            };
            Ok(InputSpec::Query { params })
        }
        other => Err(IrError {
            field: ":input".into(),
            message: "unknown input type".into(),
            expected: vec!["json".into(), "file".into(), "query".into()],
            got: other.into(),
        }),
    }
}

fn parse_output_spec(node: &SexpNode) -> Result<OutputSpec, IrError> {
    let tag = node.tag().unwrap_or("");
    match tag {
        "json" => {
            let schema = node.keyword(":schema").ok_or_else(|| IrError {
                field: ":output (json)".into(),
                message: "json output requires :schema".into(),
                expected: vec!["(json :schema TypeName)".into()],
                got: format!("{node}"),
            })?;
            Ok(OutputSpec::Json { schema: schema.into() })
        }
        "text" => Ok(OutputSpec::Text),
        other => Err(IrError {
            field: ":output".into(),
            message: "unknown output type".into(),
            expected: vec!["json".into(), "text".into()],
            got: other.into(),
        }),
    }
}

fn parse_rate_limit(spec: &str) -> Result<RateLimit, IrError> {
    let parts: Vec<&str> = spec.split('/').collect();
    if parts.len() != 2 {
        return Err(IrError {
            field: ":rate-limit".into(),
            message: "format must be 'N/period'".into(),
            expected: vec!["10/min".into(), "100/hour".into()],
            got: spec.into(),
        });
    }
    let count = parts[0].parse::<u32>().map_err(|_| IrError {
        field: ":rate-limit".into(),
        message: "count must be a number".into(),
        expected: vec!["10/min".into()],
        got: spec.into(),
    })?;
    let period = parts[1].to_string();
    if !["sec", "min", "hour", "day"].contains(&period.as_str()) {
        return Err(IrError {
            field: ":rate-limit".into(),
            message: "invalid period".into(),
            expected: vec!["sec".into(), "min".into(), "hour".into(), "day".into()],
            got: period,
        });
    }
    Ok(RateLimit { count, period })
}

fn parse_single_state_dep(s: &str) -> Result<StateDep, IrError> {
    match s {
        "db-pool" => Ok(StateDep::DbPool),
        "cache" => Ok(StateDep::Cache),
        other => Err(IrError {
            field: ":state".into(),
            message: "unknown state dependency".into(),
            expected: vec!["db-pool".into(), "cache".into()],
            got: other.into(),
        }),
    }
}

fn parse_state_deps(node: &SexpNode) -> Result<Vec<StateDep>, IrError> {
    match node {
        SexpNode::List(children) => {
            let mut deps = Vec::new();
            for child in children {
                if let SexpNode::Atom(s) = child {
                    deps.push(parse_single_state_dep(s)?);
                }
            }
            Ok(deps)
        }
        SexpNode::Atom(s) => Ok(vec![parse_single_state_dep(s)?]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_post_endpoint() {
        let sexp = SexpNode::parse_one(
            r#"(api :method POST :path "/users/me/avatar"
                 :input (file :max-size "5MB")
                 :output (json :schema UserAvatar)
                 :auth required
                 :rate-limit "10/min")"#
        ).unwrap();
        let endpoint = ApiEndpoint::from_sexp(&sexp).unwrap();
        assert_eq!(endpoint.method, HttpMethod::Post);
        assert_eq!(endpoint.path, "/users/me/avatar");
        assert_eq!(endpoint.auth, AuthRequirement::Required);
        assert!(matches!(endpoint.input, InputSpec::File { .. }));
        assert!(matches!(endpoint.output, OutputSpec::Json { .. }));
        assert_eq!(endpoint.rate_limit.as_ref().unwrap().count, 10);
    }

    #[test]
    fn valid_get_endpoint() {
        let sexp = SexpNode::parse_one(
            r#"(api :method GET :path "/health" :output text)"#
        ).unwrap();
        let endpoint = ApiEndpoint::from_sexp(&sexp).unwrap();
        assert_eq!(endpoint.method, HttpMethod::Get);
        assert_eq!(endpoint.output, OutputSpec::Text);
        assert_eq!(endpoint.auth, AuthRequirement::None);
    }

    #[test]
    fn reject_invalid_method() {
        let sexp = SexpNode::parse_one(
            r#"(api :method PATCH :path "/users")"#
        ).unwrap();
        let err = ApiEndpoint::from_sexp(&sexp).unwrap_err();
        assert_eq!(err.field, ":method");
        assert!(err.expected.contains(&"POST".to_string()));
    }

    #[test]
    fn reject_missing_path() {
        let sexp = SexpNode::parse_one(
            r#"(api :method GET)"#
        ).unwrap();
        let err = ApiEndpoint::from_sexp(&sexp).unwrap_err();
        assert_eq!(err.field, ":path");
    }

    #[test]
    fn reject_bad_path() {
        let sexp = SexpNode::parse_one(
            r#"(api :method GET :path "users")"#
        ).unwrap();
        let err = ApiEndpoint::from_sexp(&sexp).unwrap_err();
        assert_eq!(err.field, ":path");
        assert!(err.message.contains("start with '/'"));
    }

    #[test]
    fn valid_state_db_pool() {
        let sexp = SexpNode::parse_one(
            r#"(api :method PUT :path "/users/me/profile"
                 :input (json :schema UserProfile)
                 :output (json :schema UserProfile)
                 :auth required
                 :state (db-pool)
                 :description "Update user profile")"#
        ).unwrap();
        let endpoint = ApiEndpoint::from_sexp(&sexp).unwrap();
        assert_eq!(endpoint.state, vec![StateDep::DbPool]);
    }

    #[test]
    fn reject_invalid_state() {
        let sexp = SexpNode::parse_one(
            r#"(api :method GET :path "/data" :state (mongodb))"#
        ).unwrap();
        let err = ApiEndpoint::from_sexp(&sexp).unwrap_err();
        assert_eq!(err.field, ":state");
        assert!(err.expected.contains(&"db-pool".to_string()));
    }
}
