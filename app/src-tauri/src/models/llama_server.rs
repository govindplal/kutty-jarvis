//! Local models through `llama-server` sidecar processes (ADR 0012).
//!
//! Each model (speech recognition, vision) runs in its own `llama-server`,
//! started lazily on first use, bound to 127.0.0.1 on a free port, and killed
//! when the app exits. Servers the user started themselves are used through
//! `KJ_ASR_URL` / `KJ_VLM_URL` and are never started or stopped by us.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::openai_compat::{Endpoint, OpenAiCompatAsr, OpenAiCompatAssistant};
use super::{Assistant, Question, SpeechToText, shrink_for_model};

pub const LLAMA_SERVER_ENV: &str = "KJ_LLAMA_SERVER";
pub const MODELS_DIR_ENV: &str = "KJ_MODELS_DIR";
const SETUP_DOC: &str = "docs/setup/local-models.md";

const READY_TIMEOUT: Duration = Duration::from_secs(180);
/// A fresh port can be taken between picking it and llama-server binding it.
const SPAWN_ATTEMPTS: usize = 3;
const STOP_WAIT: Duration = Duration::from_secs(3);
const SHUTTING_DOWN: &str = "the app is shutting down";
const ASR_TIMEOUT: Duration = Duration::from_secs(90);
const VLM_TIMEOUT: Duration = Duration::from_secs(240);

/// One kind of local model server.
#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub label: &'static str,
    pub url_env: &'static str,
    pub model_env: &'static str,
    pub mmproj_env: &'static str,
    pub model_file: &'static str,
    pub mmproj_file: &'static str,
    pub context: u32,
}

/// LFM3-Audio-2B Q4_K_M (finding 0002).
pub const ASR: Spec = Spec {
    label: "speech recognition",
    url_env: "KJ_ASR_URL",
    model_env: "KJ_ASR_MODEL",
    mmproj_env: "KJ_ASR_MMPROJ",
    model_file: "LFM3-Audio-2B-Q4_K_M.gguf",
    mmproj_file: "mmproj-LFM3-Audio-2B-Q8_0.gguf",
    context: 4096,
};

/// LFM2.5-VL-1.6B Q4_K_M (finding 0001). The element list needs more room.
pub const VLM: Spec = Spec {
    label: "vision",
    url_env: "KJ_VLM_URL",
    model_env: "KJ_VLM_MODEL",
    mmproj_env: "KJ_VLM_MMPROJ",
    model_file: "LFM2.5-VL-1.6B-Q4_K_M.gguf",
    mmproj_file: "mmproj-LFM2.5-VL-1.6b-Q8_0.gguf",
    context: 8192,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    /// A server the user already runs.
    Existing(String),
    Spawn(SpawnPlan),
}

#[derive(Debug, PartialEq, Eq)]
pub struct SpawnPlan {
    pub exe: PathBuf,
    pub model: PathBuf,
    pub mmproj: PathBuf,
    pub context: u32,
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

/// Decides how to reach `spec` from the environment and the files present.
/// `env` and `exists` are parameters so the choice can be tested.
pub fn plan(
    spec: &Spec,
    data_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<Plan, String> {
    let get = |name: &str| {
        env(name)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    if let Some(url) = get(spec.url_env) {
        let url = url.trim_end_matches('/').to_string();
        super::require_loopback(&url).map_err(|error| format!("{}: {error}", spec.url_env))?;
        return Ok(Plan::Existing(url));
    }

    let exe = get(LLAMA_SERVER_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("llama").join(exe_name()));
    if !exists(&exe) {
        return Err(format!(
            "llama-server was not found at {}. Download llama.cpp and put {} there, or set {LLAMA_SERVER_ENV} (see {SETUP_DOC})",
            exe.display(),
            exe_name()
        ));
    }

    let models_dir = get(MODELS_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("models"));
    let file = |env_name: &str, default: &str| {
        get(env_name)
            .map(PathBuf::from)
            .unwrap_or_else(|| models_dir.join(default))
    };
    let model = file(spec.model_env, spec.model_file);
    let mmproj = file(spec.mmproj_env, spec.mmproj_file);
    for (path, default, env_name) in [
        (&model, spec.model_file, spec.model_env),
        (&mmproj, spec.mmproj_file, spec.mmproj_env),
    ] {
        if !exists(path) {
            return Err(format!(
                "the {} model file {} is missing. Put {default} in {} or set {env_name} (see {SETUP_DOC})",
                spec.label,
                path.display(),
                models_dir.display()
            ));
        }
    }
    Ok(Plan::Spawn(SpawnPlan {
        exe,
        model,
        mmproj,
        context: spec.context,
    }))
}

/// A server process we started. Killed when dropped.
struct Spawned(Child);

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Running {
    url: String,
    child: Option<Spawned>,
}

impl Running {
    fn alive(&mut self) -> bool {
        match &mut self.child {
            Some(Spawned(child)) => matches!(child.try_wait(), Ok(None)),
            None => true,
        }
    }
}

/// Starts the server on first use and restarts it if it died.
pub struct Sidecar {
    spec: Spec,
    data_dir: PathBuf,
    state: Mutex<Option<Running>>,
    /// Held while a server starts, so concurrent callers start only one
    /// without blocking `stop`, which only needs `state`.
    starting: Mutex<()>,
    shutdown: AtomicBool,
}

impl Sidecar {
    pub fn new(spec: Spec, data_dir: PathBuf) -> Self {
        Self {
            spec,
            data_dir,
            state: Mutex::new(None),
            starting: Mutex::new(()),
            shutdown: AtomicBool::new(false),
        }
    }

    fn running_url(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(running) = state.as_mut() {
            if running.alive() {
                return Some(running.url.clone());
            }
            *state = None;
        }
        None
    }

    /// Base URL of a ready server, starting one if needed. Blocks while the
    /// model loads; startup polls the shutdown flag so quitting is quick.
    pub fn url(&self) -> Result<String, String> {
        if let Some(url) = self.running_url() {
            return Ok(url);
        }
        let _starting = self.starting.lock().unwrap_or_else(|p| p.into_inner());
        // Another caller may have finished starting while we waited.
        if let Some(url) = self.running_url() {
            return Ok(url);
        }
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(SHUTTING_DOWN.to_string());
        }
        let running = self.start()?;
        let url = running.url.clone();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        // `stop` may have run during startup; dropping `running` kills it.
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(SHUTTING_DOWN.to_string());
        }
        *state = Some(running);
        Ok(url)
    }

    /// Stops the server if we started it and refuses further starts.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        *self.state.lock().unwrap_or_else(|p| p.into_inner()) = None;
        // A start in progress sees the flag within a poll and kills its
        // child; wait briefly so that happens before the process exits.
        let deadline = Instant::now() + STOP_WAIT;
        while self.starting.try_lock().is_err() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn start(&self) -> Result<Running, String> {
        let chosen = plan(
            &self.spec,
            &self.data_dir,
            &|name| std::env::var(name).ok(),
            &|path| path.is_file(),
        )?;
        match chosen {
            Plan::Existing(url) => Ok(Running { url, child: None }),
            Plan::Spawn(plan) => spawn(&self.spec, &plan, &self.shutdown),
        }
    }
}

fn free_port() -> Result<u16, String> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|error| format!("could not find a free local port: {error}"))
}

fn command_args(plan: &SpawnPlan, port: u16) -> Vec<String> {
    vec![
        "-m".into(),
        plan.model.display().to_string(),
        "--mmproj".into(),
        plan.mmproj.display().to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-c".into(),
        plan.context.to_string(),
    ]
}

/// Why one start attempt failed.
enum Attempt {
    /// The child exited, e.g. because another process took the port.
    Exited(String),
    Fatal(String),
}

fn spawn(spec: &Spec, plan: &SpawnPlan, shutdown: &AtomicBool) -> Result<Running, String> {
    let deadline = Instant::now() + READY_TIMEOUT;
    let mut last = String::new();
    for _ in 0..SPAWN_ATTEMPTS {
        match spawn_once(spec, plan, shutdown, deadline) {
            Ok(running) => return Ok(running),
            Err(Attempt::Exited(message)) => last = message,
            Err(Attempt::Fatal(message)) => return Err(message),
        }
    }
    Err(last)
}

fn spawn_once(
    spec: &Spec,
    plan: &SpawnPlan,
    shutdown: &AtomicBool,
    deadline: Instant,
) -> Result<Running, Attempt> {
    let port = free_port().map_err(Attempt::Fatal)?;
    let mut command = Command::new(&plan.exe);
    command
        .args(command_args(plan, port))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up next to the app.
        command.creation_flags(0x0800_0000);
    }
    let child = command.spawn().map_err(|error| {
        Attempt::Fatal(format!(
            "could not start llama-server at {}: {error}",
            plan.exe.display()
        ))
    })?;
    let mut spawned = Spawned(child);
    let url = format!("http://127.0.0.1:{port}");
    eprintln!("started llama-server for {} on {url}", spec.label);

    let exited = |status| {
        Attempt::Exited(format!(
            "llama-server for {} stopped right after starting ({status}); the model file may be damaged or the computer short of memory",
            spec.label
        ))
    };
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Err(Attempt::Fatal(SHUTTING_DOWN.to_string()));
        }
        if let Ok(Some(status)) = spawned.0.try_wait() {
            return Err(exited(status));
        }
        // Someone else may own the port and answer /health, so the child
        // must still be alive once it reports ready.
        if is_ready(&url) {
            return match spawned.0.try_wait() {
                Ok(None) => Ok(Running {
                    url,
                    child: Some(spawned),
                }),
                Ok(Some(status)) => Err(exited(status)),
                Err(error) => Err(Attempt::Fatal(format!(
                    "could not check llama-server for {}: {error}",
                    spec.label
                ))),
            };
        }
        if Instant::now() >= deadline {
            return Err(Attempt::Fatal(format!(
                "the {} model did not finish loading within {} seconds",
                spec.label,
                READY_TIMEOUT.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn is_ready(url: &str) -> bool {
    super::local_agent(None, Duration::from_secs(2))
        .get(format!("{url}/health"))
        .call()
        .is_ok_and(|response| response.status().as_u16() == 200)
}

/// The two local servers, shared by the whole app.
pub struct LlamaServers {
    asr: Arc<Sidecar>,
    vlm: Arc<Sidecar>,
}

impl LlamaServers {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            asr: Arc::new(Sidecar::new(ASR, data_dir.clone())),
            vlm: Arc::new(Sidecar::new(VLM, data_dir)),
        }
    }

    pub fn speech_to_text(&self) -> Arc<dyn SpeechToText> {
        Arc::new(LocalAsr(Arc::clone(&self.asr)))
    }

    pub fn assistant(&self) -> Arc<dyn Assistant> {
        Arc::new(LocalVision(Arc::clone(&self.vlm)))
    }

    pub fn stop_all(&self) {
        self.asr.stop();
        self.vlm.stop();
    }
}

struct LocalAsr(Arc<Sidecar>);

impl SpeechToText for LocalAsr {
    fn transcribe(&self, wav: &[u8]) -> Result<String, String> {
        let endpoint = Endpoint::local(self.0.url()?, ASR_TIMEOUT);
        OpenAiCompatAsr(endpoint).transcribe(wav)
    }
}

struct LocalVision(Arc<Sidecar>);

impl Assistant for LocalVision {
    fn answer(&self, question: &Question) -> Result<String, String> {
        let endpoint = Endpoint::local(self.0.url()?, VLM_TIMEOUT);
        OpenAiCompatAssistant(endpoint).answer(&shrink_for_model(question))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn plan_with(spec: &Spec, env: &[(&str, &str)], present: &[&str]) -> Result<Plan, String> {
        let env: HashMap<String, String> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let present: Vec<PathBuf> = present.iter().map(PathBuf::from).collect();
        plan(
            spec,
            Path::new("/data"),
            &|name| env.get(name).cloned(),
            &|path| present.iter().any(|p| p == path),
        )
    }

    fn default_files() -> Vec<String> {
        vec![
            Path::new("/data")
                .join("llama")
                .join(exe_name())
                .display()
                .to_string(),
            Path::new("/data")
                .join("models")
                .join(VLM.model_file)
                .display()
                .to_string(),
            Path::new("/data")
                .join("models")
                .join(VLM.mmproj_file)
                .display()
                .to_string(),
        ]
    }

    #[test]
    fn a_url_in_the_environment_wins_and_is_trimmed() {
        let chosen = plan_with(&VLM, &[("KJ_VLM_URL", " http://127.0.0.1:8089/ ")], &[]);
        assert_eq!(chosen, Ok(Plan::Existing("http://127.0.0.1:8089".into())));
    }

    #[test]
    fn a_url_override_must_be_local() {
        let error = plan_with(&ASR, &[("KJ_ASR_URL", "http://example.com:8080")], &[]).unwrap_err();
        assert!(error.contains("KJ_ASR_URL") && error.contains("not a local address"));
    }

    #[test]
    fn no_server_starts_after_stop() {
        let sidecar = Sidecar::new(VLM, PathBuf::from("/nowhere"));
        sidecar.stop();
        assert_eq!(sidecar.url().unwrap_err(), SHUTTING_DOWN);
    }

    #[test]
    fn blank_url_is_ignored() {
        let error = plan_with(&VLM, &[("KJ_VLM_URL", "  ")], &[]).unwrap_err();
        assert!(error.contains("llama-server was not found"), "{error}");
    }

    #[test]
    fn defaults_live_under_the_app_data_folder() {
        let files = default_files();
        let present: Vec<&str> = files.iter().map(String::as_str).collect();
        let Ok(Plan::Spawn(spawn)) = plan_with(&VLM, &[], &present) else {
            panic!("expected a spawn plan");
        };
        assert_eq!(spawn.exe, Path::new("/data").join("llama").join(exe_name()));
        assert_eq!(
            spawn.model,
            Path::new("/data").join("models").join(VLM.model_file)
        );
        assert_eq!(spawn.context, 8192);
    }

    #[test]
    fn missing_model_names_the_folder_and_file() {
        let files = default_files();
        let present = [files[0].as_str()];
        let error = plan_with(&ASR, &[], &present).unwrap_err();
        assert!(error.contains("speech recognition"), "{error}");
        assert!(error.contains(ASR.model_file), "{error}");
        assert!(error.contains(&Path::new("/data").join("models").display().to_string()));
        assert!(error.contains("KJ_ASR_MODEL"), "{error}");
    }

    #[test]
    fn missing_projector_is_reported_separately() {
        let files = default_files();
        let present = [files[0].as_str(), files[1].as_str()];
        let error = plan_with(&VLM, &[], &present).unwrap_err();
        assert!(error.contains(VLM.mmproj_file), "{error}");
    }

    #[test]
    fn environment_overrides_paths() {
        let present = ["/opt/ls", "/m/a.gguf", "/m/b.gguf"];
        let chosen = plan_with(
            &VLM,
            &[
                ("KJ_LLAMA_SERVER", "/opt/ls"),
                ("KJ_VLM_MODEL", "/m/a.gguf"),
                ("KJ_VLM_MMPROJ", "/m/b.gguf"),
            ],
            &present,
        );
        assert_eq!(
            chosen,
            Ok(Plan::Spawn(SpawnPlan {
                exe: "/opt/ls".into(),
                model: "/m/a.gguf".into(),
                mmproj: "/m/b.gguf".into(),
                context: 8192,
            }))
        );
    }

    #[test]
    fn models_dir_override_moves_the_defaults() {
        let under = |dir: &str, name: &str| Path::new(dir).join(name).display().to_string();
        let exe = under("/data/llama", exe_name());
        let model = under("/elsewhere", ASR.model_file);
        let mmproj = under("/elsewhere", ASR.mmproj_file);
        let present = [exe.as_str(), model.as_str(), mmproj.as_str()];
        let Ok(Plan::Spawn(spawn)) = plan_with(&ASR, &[("KJ_MODELS_DIR", "/elsewhere")], &present)
        else {
            panic!("expected a spawn plan");
        };
        assert_eq!(spawn.model, Path::new(&model));
    }

    #[test]
    fn server_listens_on_localhost_only() {
        let args = command_args(
            &SpawnPlan {
                exe: "x".into(),
                model: "m.gguf".into(),
                mmproj: "p.gguf".into(),
                context: 4096,
            },
            5555,
        );
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(at("--host").as_deref(), Some("127.0.0.1"));
        assert_eq!(at("--port").as_deref(), Some("5555"));
        assert_eq!(at("-m").as_deref(), Some("m.gguf"));
        assert_eq!(at("--mmproj").as_deref(), Some("p.gguf"));
    }

    #[test]
    fn free_port_is_usable() {
        assert!(free_port().unwrap() > 0);
    }
}
