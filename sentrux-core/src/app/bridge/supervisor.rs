//! Persistent Node bridge supervision for TypeScript semantic analysis.

use crate::analysis::semantic::typescript::{
    analyze_project_request, bridge_command, default_bridge_config, initialize_request,
    ping_request, shutdown_request, BridgeInitializeResult, TypeScriptBridgeCommand,
    TypeScriptBridgeConfig, TypeScriptBridgeError, TypeScriptBridgeRequest,
    TypeScriptBridgeResponse, TS_BRIDGE_PROTOCOL_VERSION,
};
use crate::analysis::semantic::{ProjectModel, SemanticSnapshot};
use serde::de::DeserializeOwned;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Debug)]
pub enum BridgeError {
    Unavailable(String),
    Io(String),
    Protocol(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Io(message) => write!(f, "{message}"),
            Self::Protocol(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for BridgeError {}

pub struct TypeScriptBridgeSupervisor {
    config: TypeScriptBridgeConfig,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_request_id: u64,
    initialized: bool,
    capabilities: Option<BridgeInitializeResult>,
}

impl TypeScriptBridgeSupervisor {
    pub fn new(config: TypeScriptBridgeConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            stdout: None,
            next_request_id: 1,
            initialized: false,
            capabilities: None,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(default_bridge_config())
    }

    pub fn config(&self) -> &TypeScriptBridgeConfig {
        &self.config
    }

    pub fn capabilities(&self) -> Option<&BridgeInitializeResult> {
        self.capabilities.as_ref()
    }

    pub fn start(&mut self) -> Result<(), BridgeError> {
        if self.initialized && self.is_running() {
            return Ok(());
        }

        if self.child.is_some() {
            self.terminate_process()?;
        }

        if !Path::new(&self.config.entrypoint).is_file() {
            return Err(BridgeError::Unavailable(format!(
                "TypeScript bridge entrypoint is unavailable: {}",
                self.config.entrypoint
            )));
        }

        let command = bridge_command(&self.config);
        let mut child = spawn_bridge_process(&command)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            BridgeError::Io("TypeScript bridge started without stdin pipe".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            BridgeError::Io("TypeScript bridge started without stdout pipe".to_string())
        })?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.child = Some(child);
        self.initialized = false;
        self.capabilities = None;

        let request_id = self.next_id();
        let response = match self.send_request(initialize_request(request_id)) {
            Ok(response) => response,
            Err(error) => {
                let _ = self.terminate_process();
                return Err(error);
            }
        };
        let initialize: BridgeInitializeResult = match parse_result(response) {
            Ok(initialize) => initialize,
            Err(error) => {
                let _ = self.terminate_process();
                return Err(error);
            }
        };
        if initialize.protocol_version != TS_BRIDGE_PROTOCOL_VERSION {
            let _ = self.terminate_process();
            return Err(BridgeError::Protocol(format!(
                "TypeScript bridge protocol mismatch: expected {}, got {}",
                TS_BRIDGE_PROTOCOL_VERSION, initialize.protocol_version
            )));
        }
        self.capabilities = Some(initialize);
        self.initialized = true;

        Ok(())
    }

    pub fn is_running(&mut self) -> bool {
        let running = match self.child.as_mut() {
            Some(child) => child.try_wait().ok().flatten().is_none(),
            None => false,
        };
        if !running {
            self.reset_handles();
        }
        running
    }

    pub fn ensure_started(&mut self) -> Result<(), BridgeError> {
        if self.initialized && self.is_running() {
            return Ok(());
        }

        self.start()
    }

    pub fn ping(&mut self) -> Result<(), BridgeError> {
        self.ensure_started()?;
        let request_id = self.next_id();
        let response = self.send_request(ping_request(request_id))?;
        ensure_success(response)
    }

    pub fn analyze_project(
        &mut self,
        project: &ProjectModel,
    ) -> Result<SemanticSnapshot, BridgeError> {
        self.ensure_started()?;
        let request_id = self.next_id();
        let response = self.send_request(analyze_project_request(request_id, project))?;

        parse_result(response)
    }

    pub fn shutdown(&mut self) -> Result<(), BridgeError> {
        if !self.is_running() {
            self.reset_handles();
            return Ok(());
        }

        let request_id = self.next_id();
        let shutdown_result = self
            .send_request(shutdown_request(request_id))
            .and_then(ensure_success);
        let kill_result = self.terminate_process();

        match shutdown_result {
            Ok(()) => kill_result,
            Err(error) => {
                let _ = kill_result;
                Err(error)
            }
        }
    }

    fn send_request(
        &mut self,
        request: TypeScriptBridgeRequest,
    ) -> Result<TypeScriptBridgeResponse, BridgeError> {
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            BridgeError::Protocol("TypeScript bridge stdin is not available".to_string())
        })?;
        let stdout = self.stdout.as_mut().ok_or_else(|| {
            BridgeError::Protocol("TypeScript bridge stdout is not available".to_string())
        })?;
        let request_id = request.id;

        write_request(stdin, &request)?;
        let response = read_response(stdout)?;
        validate_response(&response, request_id)?;

        Ok(response)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    fn reset_handles(&mut self) {
        self.child = None;
        self.stdin = None;
        self.stdout = None;
        self.initialized = false;
        self.capabilities = None;
    }
    fn terminate_process(&mut self) -> Result<(), BridgeError> {
        let result = terminate_child(self.child.as_mut());
        self.reset_handles();
        result
    }
}

impl Drop for TypeScriptBridgeSupervisor {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn spawn_bridge_process(command: &TypeScriptBridgeCommand) -> Result<Child, BridgeError> {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .current_dir(&command.current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    for (key, value) in &command.env {
        process.env(key, value);
    }

    process.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            BridgeError::Unavailable(format!(
                "TypeScript bridge program '{}' is unavailable: {error}",
                command.program
            ))
        } else {
            BridgeError::Io(format!(
                "Failed to start TypeScript bridge '{}': {error}",
                command.program
            ))
        }
    })
}

fn write_request(
    stdin: &mut ChildStdin,
    request: &TypeScriptBridgeRequest,
) -> Result<(), BridgeError> {
    let body = serde_json::to_vec(request).map_err(|error| {
        BridgeError::Protocol(format!("Failed to encode bridge request: {error}"))
    })?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());

    stdin
        .write_all(header.as_bytes())
        .map_err(|error| BridgeError::Io(format!("Failed to write bridge header: {error}")))?;
    stdin
        .write_all(&body)
        .map_err(|error| BridgeError::Io(format!("Failed to write bridge body: {error}")))?;
    stdin
        .flush()
        .map_err(|error| BridgeError::Io(format!("Failed to flush bridge request: {error}")))?;

    Ok(())
}

fn read_response(
    stdout: &mut BufReader<ChildStdout>,
) -> Result<TypeScriptBridgeResponse, BridgeError> {
    let content_length = read_content_length(stdout)?;
    let mut body = vec![0_u8; content_length];
    stdout
        .read_exact(&mut body)
        .map_err(|error| BridgeError::Io(format!("Failed to read bridge body: {error}")))?;

    serde_json::from_slice(&body).map_err(|error| {
        BridgeError::Protocol(format!("Failed to decode bridge response: {error}"))
    })
}

fn read_content_length(stdout: &mut BufReader<ChildStdout>) -> Result<usize, BridgeError> {
    let mut line = String::new();
    let mut content_length = None;

    loop {
        line.clear();
        let bytes = stdout
            .read_line(&mut line)
            .map_err(|error| BridgeError::Io(format!("Failed to read bridge header: {error}")))?;
        if bytes == 0 {
            return Err(BridgeError::Protocol(
                "TypeScript bridge closed before sending a response".to_string(),
            ));
        }

        if line == "\r\n" {
            break;
        }

        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                let parsed = value.trim().parse::<usize>().map_err(|error| {
                    BridgeError::Protocol(format!("Invalid Content-Length header: {error}"))
                })?;
                content_length = Some(parsed);
            }
        }
    }

    content_length.ok_or_else(|| {
        BridgeError::Protocol("TypeScript bridge response omitted Content-Length".to_string())
    })
}

fn validate_response(
    response: &TypeScriptBridgeResponse,
    request_id: u64,
) -> Result<(), BridgeError> {
    if response.jsonrpc != "2.0" {
        return Err(BridgeError::Protocol(format!(
            "TypeScript bridge returned unsupported JSON-RPC version: {}",
            response.jsonrpc
        )));
    }
    if response.id != request_id {
        return Err(BridgeError::Protocol(format!(
            "TypeScript bridge response id mismatch: expected {request_id}, got {}",
            response.id
        )));
    }

    Ok(())
}

fn parse_result<T>(response: TypeScriptBridgeResponse) -> Result<T, BridgeError>
where
    T: DeserializeOwned,
{
    if let Some(error) = response.error {
        return Err(bridge_protocol_error(error));
    }

    let result = response.result.ok_or_else(|| {
        BridgeError::Protocol("TypeScript bridge response omitted result payload".to_string())
    })?;
    serde_json::from_value(result).map_err(|error| {
        BridgeError::Protocol(format!("Failed to decode bridge result payload: {error}"))
    })
}

fn ensure_success(response: TypeScriptBridgeResponse) -> Result<(), BridgeError> {
    if let Some(error) = response.error {
        return Err(bridge_protocol_error(error));
    }

    Ok(())
}

fn bridge_protocol_error(error: TypeScriptBridgeError) -> BridgeError {
    let detail = error
        .data
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(|value| value.as_str())
        .map(|message| format!(" ({message})"))
        .unwrap_or_default();
    BridgeError::Protocol(format!(
        "TypeScript bridge error {}: {}{}",
        error.code, error.message, detail
    ))
}

fn terminate_child(child: Option<&mut Child>) -> Result<(), BridgeError> {
    let Some(child) = child else {
        return Ok(());
    };

    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => {
            return Err(BridgeError::Io(format!(
                "Failed to terminate TypeScript bridge: {error}"
            )));
        }
    }

    child.wait().map_err(|error| {
        BridgeError::Io(format!(
            "Failed to wait for TypeScript bridge shutdown: {error}"
        ))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TypeScriptBridgeSupervisor;
    use crate::analysis::semantic::{discover_project, SemanticCapability};
    use crate::analysis::semantic::typescript::TypeScriptBridgeConfig;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sentrux-bridge-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(root: &std::path::Path, relative_path: &str, contents: &str) {
        let absolute_path = root.join(relative_path);
        if let Some(parent) = absolute_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(&absolute_path, contents).expect("write file");
    }

    #[test]
    fn missing_node_binary_is_reported_as_unavailable() {
        let mut supervisor = TypeScriptBridgeSupervisor::new(TypeScriptBridgeConfig {
            node_binary: "definitely-not-a-real-node-binary".to_string(),
            package_dir: ".".to_string(),
            entrypoint: "missing.js".to_string(),
        });

        let error = supervisor.start().expect_err("missing binary should fail");
        assert!(error.to_string().contains("unavailable"));
    }

    #[test]
    fn supervisor_starts_and_pings_scaffold_bridge() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        supervisor.start().expect("bridge should start");
        supervisor.ping().expect("bridge should answer ping");

        let capabilities = supervisor.capabilities().expect("bridge capabilities");
        assert_eq!(capabilities.protocol_version, "0.1.0");
    }

    #[test]
    fn supervisor_analyzes_typescript_project() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");
        let project = discover_project(root).expect("project discovery");
        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        let snapshot = supervisor
            .analyze_project(&project)
            .expect("semantic analysis");

        assert_eq!(
            snapshot.project.primary_language.as_deref(),
            Some("typescript")
        );
        assert!(snapshot.analyzed_files >= 1);
        assert!(snapshot.capabilities.iter().any(|capability| matches!(
            capability,
            crate::analysis::semantic::SemanticCapability::Symbols
        )));
    }

    #[test]
    fn supervisor_collects_nested_object_property_symbols() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let root = temp_root("property-symbols");
        write_file(
            &root,
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true
  },
  "include": ["src/**/*.ts"]
}
"#,
        );
        write_file(
            &root,
            "src/store/core.ts",
            "export const store = { taskGitStatus: 'idle', nested: { branch: 1 } };\n",
        );

        let project = discover_project(&root).expect("project discovery");
        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        let snapshot = supervisor
            .analyze_project(&project)
            .expect("semantic analysis");
        let symbol_names = snapshot
            .symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();

        assert!(symbol_names.contains(&"store"));
        assert!(symbol_names.contains(&"store.taskGitStatus"));
        assert!(symbol_names.contains(&"store.nested"));
        assert!(symbol_names.contains(&"store.nested.branch"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supervisor_collects_if_transition_sites() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let root = temp_root("transition-sites");
        write_file(
            &root,
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true
  },
  "include": ["src/**/*.ts"]
}
"#,
        );
        write_file(
            &root,
            "src/runtime/browser-state.ts",
            r#"export type BrowserSyncState = "idle" | "running" | "error";

export function nextState(state: BrowserSyncState): BrowserSyncState {
  if (state === "idle") {
    return "running";
  } else if (state === "running") {
    return "error";
  } else {
    return "idle";
  }
}
"#,
        );

        let project = discover_project(&root).expect("project discovery");
        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        let snapshot = supervisor
            .analyze_project(&project)
            .expect("semantic analysis");

        assert!(snapshot
            .capabilities
            .iter()
            .any(|capability| matches!(capability, SemanticCapability::TransitionSites)));
        assert_eq!(snapshot.transition_sites.len(), 3);
        assert_eq!(snapshot.transition_sites[0].source_variant.as_deref(), Some("idle"));
        assert!(snapshot.transition_sites[0]
            .target_variants
            .contains(&"running".to_string()));
        assert_eq!(snapshot.transition_sites[1].source_variant.as_deref(), Some("running"));
        assert!(snapshot.transition_sites[1]
            .target_variants
            .contains(&"error".to_string()));
        assert_eq!(snapshot.transition_sites[2].source_variant.as_deref(), Some("error"));
        assert!(snapshot.transition_sites[2]
            .target_variants
            .contains(&"idle".to_string()));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supervisor_skips_switches_without_explicit_next_state_mappings() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let root = temp_root("transition-free-switch");
        write_file(
            &root,
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true
  },
  "include": ["src/**/*.ts"]
}
"#,
        );
        write_file(
            &root,
            "src/runtime/browser-state.ts",
            r#"export type BrowserSyncState = "idle" | "running";

export function renderStateLabel(state: BrowserSyncState): string {
  switch (state) {
    case "idle":
      return "Idle";
    case "running":
      return "Running";
  }
}
"#,
        );

        let project = discover_project(&root).expect("project discovery");
        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        let snapshot = supervisor
            .analyze_project(&project)
            .expect("semantic analysis");

        assert!(snapshot.transition_sites.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supervisor_keeps_transition_groups_with_domain_typed_returns() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let root = temp_root("transition-helper-switch");
        write_file(
            &root,
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true
  },
  "include": ["src/**/*.ts"]
}
"#,
        );
        write_file(
            &root,
            "src/runtime/browser-state.ts",
            r#"export type BrowserSyncState = "idle" | "running";

function advance(current: BrowserSyncState): BrowserSyncState {
  return current === "idle" ? "running" : "idle";
}

export function nextState(state: BrowserSyncState): BrowserSyncState {
  switch (state) {
    case "idle":
      return advance(state);
    case "running":
      return advance(state);
  }
}
"#,
        );

        let project = discover_project(&root).expect("project discovery");
        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        let snapshot = supervisor
            .analyze_project(&project)
            .expect("semantic analysis");

        assert_eq!(snapshot.transition_sites.len(), 2);
        assert!(snapshot
            .transition_sites
            .iter()
            .all(|site| site.target_variants.is_empty()));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supervisor_collects_binary_state_inequality_transitions() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let root = temp_root("transition-inequality-if");
        write_file(
            &root,
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true
  },
  "include": ["src/**/*.ts"]
}
"#,
        );
        write_file(
            &root,
            "src/runtime/browser-state.ts",
            r#"export type BrowserSyncState = "idle" | "running";

export function nextState(state: BrowserSyncState): BrowserSyncState {
  if (state !== "idle") {
    return "idle";
  } else {
    return "running";
  }
}
"#,
        );

        let project = discover_project(&root).expect("project discovery");
        let mut supervisor = TypeScriptBridgeSupervisor::with_default_config();
        let snapshot = supervisor
            .analyze_project(&project)
            .expect("semantic analysis");

        assert_eq!(snapshot.transition_sites.len(), 2);
        assert_eq!(
            snapshot.transition_sites[0].source_variant.as_deref(),
            Some("running")
        );
        assert!(snapshot.transition_sites[0]
            .target_variants
            .contains(&"idle".to_string()));
        assert_eq!(
            snapshot.transition_sites[1].source_variant.as_deref(),
            Some("idle")
        );
        assert!(snapshot.transition_sites[1]
            .target_variants
            .contains(&"running".to_string()));

        let _ = std::fs::remove_dir_all(root);
    }
}
