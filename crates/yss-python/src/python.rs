//! Project Python scripts: per-project virtual environments made with uv or the system Python,
//! script files under `<project>/scripts`, and runs with a deadline, cancellation and bounded output.
//! Every function blocks; callers run them off their async executors.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use rig_tools::process::{Ended, Launcher, OutputLine, Spawn};
use serde::Deserialize;
use yss_harness_contract::{
    CancellationToken, PythonEnvironment, PythonFailure, PythonOutputFile, PythonPackage,
    PythonPackagesInstalled, PythonRun, PythonRunScriptRequest, PythonScript, PythonScriptWritten,
    python_script_file,
};

const SCRIPTS_DIRECTORY: &str = "scripts";
const OUTPUT_DIRECTORY: &str = "output";
const REQUIREMENTS_FILE: &str = "requirements.txt";
const HEAD_LINES: usize = 80;
const TAIL_LINES: usize = 300;
const TAIL_BYTES: usize = 48 * 1024;
const INSTALL_TIMEOUT: Duration = Duration::from_secs(900);
const SETUP: &str = "Python is not installed. Ask the user to install uv (https://docs.astral.sh/uv/getting-started/installation/) or Python 3, then restart YssBI.";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Toolchain {
    Uv(PathBuf),
    System(PathBuf),
}

pub struct PythonRuntime {
    environments: PathBuf,
}

impl PythonRuntime {
    /// `environments` holds one virtual environment per project folder, outside the project.
    pub fn new(environments: PathBuf) -> Self {
        Self { environments }
    }

    pub fn inspect(&self, project: &Path) -> Result<PythonEnvironment, PythonFailure> {
        let output_directory = output_directory(project);
        let scripts = list_scripts(&scripts_directory(project))?;
        let Some(toolchain) = toolchain() else {
            return Ok(PythonEnvironment {
                available: false,
                manager: None,
                python_version: None,
                packages: Vec::new(),
                scripts,
                output_directory: output_directory.display().to_string(),
                setup: Some(SETUP.to_owned()),
            });
        };
        let environment = self.environment(project);
        let interpreter = environment
            .exists()
            .then(|| venv_python(&environment))
            .filter(|python| python.exists());
        let python_version = match (&interpreter, &toolchain) {
            (Some(python), _) | (None, Toolchain::System(python)) => version(python),
            (None, Toolchain::Uv(_)) => None,
        };
        let packages = match &interpreter {
            Some(python) => packages(&toolchain, python).unwrap_or_default(),
            None => Vec::new(),
        };
        Ok(PythonEnvironment {
            available: true,
            manager: Some(
                match toolchain {
                    Toolchain::Uv(_) => "uv",
                    Toolchain::System(_) => "venv",
                }
                .to_owned(),
            ),
            python_version,
            packages,
            scripts,
            output_directory: output_directory.display().to_string(),
            setup: None,
        })
    }

    pub fn read_script(&self, project: &Path, name: &str) -> Result<PythonScript, PythonFailure> {
        let path = scripts_directory(project).join(python_script_file(name));
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(PythonScript {
                name: python_script_file(name),
                content,
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(PythonFailure::ScriptNotFound)
            }
            Err(_) => Err(PythonFailure::ProjectUnavailable),
        }
    }

    pub fn write_script(
        &self,
        project: &Path,
        name: &str,
        content: &str,
    ) -> Result<PythonScriptWritten, PythonFailure> {
        let directory = scripts_directory(project);
        std::fs::create_dir_all(&directory).map_err(|_| PythonFailure::ProjectUnavailable)?;
        let path = directory.join(python_script_file(name));
        std::fs::write(&path, content).map_err(|_| PythonFailure::ProjectUnavailable)?;
        Ok(PythonScriptWritten {
            name: python_script_file(name),
            path: path.display().to_string(),
            bytes: content.len(),
        })
    }

    pub fn install_packages(
        &self,
        project: &Path,
        packages: &[String],
        cancel: &CancellationToken,
    ) -> Result<PythonPackagesInstalled, PythonFailure> {
        let toolchain = available_toolchain()?;
        let python = self.ensure_environment(&toolchain, project, cancel)?;
        let packages: Vec<String> = packages
            .iter()
            .map(|package| package.trim().to_owned())
            .collect();
        let (program, mut arguments) = install_command(&toolchain, &python);
        arguments.extend(packages.iter().map(OsString::from));
        let run = run_process(&program, &arguments, project, &[], INSTALL_TIMEOUT, cancel)?;
        if run.exit_code != Some(0) {
            return Err(PythonFailure::InstallFailed { output: run.output });
        }
        let requirements = record_requirements(&scripts_directory(project), &packages)?;
        Ok(PythonPackagesInstalled {
            requirements,
            output: run.output,
        })
    }

    pub fn run_script(
        &self,
        project: &Path,
        request: &PythonRunScriptRequest,
        cancel: &CancellationToken,
    ) -> Result<PythonRun, PythonFailure> {
        let scripts = scripts_directory(project);
        let script = scripts.join(python_script_file(&request.name));
        if !script.is_file() {
            return Err(PythonFailure::ScriptNotFound);
        }
        let toolchain = available_toolchain()?;
        let python = self.ensure_environment(&toolchain, project, cancel)?;
        let output = output_directory(project);
        std::fs::create_dir_all(&output).map_err(|_| PythonFailure::ProjectUnavailable)?;
        let before = snapshot(&output);
        let mut arguments = vec![OsString::from("-u"), script.into_os_string()];
        arguments.extend(request.arguments.iter().map(OsString::from));
        let environment = [
            ("YSSBI_OUTPUT_DIR", output.clone().into_os_string()),
            ("PYTHONIOENCODING", "utf-8".into()),
            ("PYTHONUTF8", "1".into()),
            ("MPLBACKEND", "Agg".into()),
        ];
        let mut run = run_process(
            python.as_os_str(),
            &arguments,
            &scripts,
            &environment,
            Duration::from_secs(u64::from(request.timeout_seconds)),
            cancel,
        )?;
        run.output_files = changed_files(&output, &before);
        Ok(run)
    }

    fn environment(&self, project: &Path) -> PathBuf {
        let key = yss_canonical_hash::content_sha256(project.as_os_str().as_encoded_bytes());
        self.environments.join(key.get(..24).unwrap_or(&key))
    }

    /// The environment's interpreter, creating the environment and installing the project's
    /// recorded requirements on first use.
    fn ensure_environment(
        &self,
        toolchain: &Toolchain,
        project: &Path,
        cancel: &CancellationToken,
    ) -> Result<PathBuf, PythonFailure> {
        let environment = self.environment(project);
        let python = venv_python(&environment);
        if python.exists() {
            return Ok(python);
        }
        std::fs::create_dir_all(&self.environments).map_err(|_| PythonFailure::Failed)?;
        let (program, arguments) = match toolchain {
            Toolchain::Uv(uv) => (
                uv.clone().into_os_string(),
                vec!["venv".into(), environment.clone().into_os_string()],
            ),
            Toolchain::System(system) => (
                system.clone().into_os_string(),
                vec![
                    "-m".into(),
                    "venv".into(),
                    environment.clone().into_os_string(),
                ],
            ),
        };
        let created = run_process(&program, &arguments, project, &[], INSTALL_TIMEOUT, cancel)?;
        if created.exit_code != Some(0) || !python.exists() {
            // A half-made environment would be mistaken for a ready one next time.
            if let Err(error) = std::fs::remove_dir_all(&environment) {
                tracing::debug!(%error, "Incomplete Python environment was not removed");
            }
            return Err(PythonFailure::InstallFailed {
                output: created.output,
            });
        }
        let requirements = scripts_directory(project).join(REQUIREMENTS_FILE);
        if requirements.is_file() {
            let (program, mut arguments) = install_command(toolchain, &python);
            arguments.extend(["-r".into(), requirements.into_os_string()]);
            let installed =
                run_process(&program, &arguments, project, &[], INSTALL_TIMEOUT, cancel)?;
            if installed.exit_code != Some(0) {
                return Err(PythonFailure::InstallFailed {
                    output: installed.output,
                });
            }
        }
        Ok(python)
    }
}

fn scripts_directory(project: &Path) -> PathBuf {
    project.join(SCRIPTS_DIRECTORY)
}

fn output_directory(project: &Path) -> PathBuf {
    scripts_directory(project).join(OUTPUT_DIRECTORY)
}

fn venv_python(environment: &Path) -> PathBuf {
    if cfg!(windows) {
        environment.join("Scripts").join("python.exe")
    } else {
        environment.join("bin").join("python")
    }
}

fn available_toolchain() -> Result<Toolchain, PythonFailure> {
    toolchain().ok_or_else(|| PythonFailure::Unavailable {
        setup: SETUP.to_owned(),
    })
}

fn toolchain() -> Option<Toolchain> {
    if let Some(uv) = find_uv() {
        return Some(Toolchain::Uv(uv));
    }
    ["python3", "python"]
        .into_iter()
        .filter_map(find_executable)
        .find(|python| version(python).is_some_and(|version| version.starts_with("3.")))
        .map(Toolchain::System)
}

fn find_uv() -> Option<PathBuf> {
    find_executable("uv").or_else(|| {
        // The uv installer's default locations are often missing from a desktop app's PATH.
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
        let home = PathBuf::from(home);
        [
            home.join(".local").join("bin"),
            home.join(".cargo").join("bin"),
        ]
        .into_iter()
        .map(|directory| directory.join(executable_name("uv")))
        .find(|path| path.is_file())
    })
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(executable_name(name)))
        .find(|candidate| candidate.is_file())
}

fn version(python: &Path) -> Option<String> {
    let output = std::process::Command::new(python)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    })
    .into_owned();
    text.trim().strip_prefix("Python ").map(str::to_owned)
}

fn install_command(toolchain: &Toolchain, python: &Path) -> (OsString, Vec<OsString>) {
    match toolchain {
        Toolchain::Uv(uv) => (
            uv.clone().into_os_string(),
            vec![
                "pip".into(),
                "install".into(),
                "--python".into(),
                python.as_os_str().to_owned(),
            ],
        ),
        Toolchain::System(_) => (
            python.as_os_str().to_owned(),
            vec!["-m".into(), "pip".into(), "install".into()],
        ),
    }
}

#[derive(Deserialize)]
struct ListedPackage {
    name: String,
    version: String,
}

fn packages(toolchain: &Toolchain, python: &Path) -> Option<Vec<PythonPackage>> {
    let mut command = match toolchain {
        Toolchain::Uv(uv) => {
            let mut command = std::process::Command::new(uv);
            command
                .args(["pip", "list", "--format", "json", "--python"])
                .arg(python);
            command
        }
        Toolchain::System(_) => {
            let mut command = std::process::Command::new(python);
            command.args(["-m", "pip", "list", "--format", "json"]);
            command
        }
    };
    let output = command.output().ok()?;
    let listed: Vec<ListedPackage> = serde_json::from_slice(&output.stdout).ok()?;
    Some(
        listed
            .into_iter()
            .map(|package| PythonPackage {
                name: package.name,
                version: package.version,
            })
            .collect(),
    )
}

fn list_scripts(directory: &Path) -> Result<Vec<String>, PythonFailure> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(PythonFailure::ProjectUnavailable),
    };
    let mut scripts: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".py"))
        .collect();
    scripts.sort();
    Ok(scripts)
}

/// Adds `packages` to the project's requirements, replacing earlier specifiers of the same
/// distribution, and returns the file's lines.
fn record_requirements(scripts: &Path, packages: &[String]) -> Result<Vec<String>, PythonFailure> {
    std::fs::create_dir_all(scripts).map_err(|_| PythonFailure::ProjectUnavailable)?;
    let path = scripts.join(REQUIREMENTS_FILE);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(PythonFailure::ProjectUnavailable),
    };
    let distribution = |specifier: &str| {
        specifier
            .split(|character: char| "[<>=!~,; ".contains(character))
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .replace('_', "-")
    };
    let mut requirements: BTreeMap<String, String> = existing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| (distribution(line), line.to_owned()))
        .collect();
    for package in packages {
        requirements.insert(distribution(package), package.clone());
    }
    let lines: Vec<String> = requirements.into_values().collect();
    std::fs::write(&path, format!("{}\n", lines.join("\n")))
        .map_err(|_| PythonFailure::ProjectUnavailable)?;
    Ok(lines)
}

fn snapshot(directory: &Path) -> BTreeMap<PathBuf, (u64, Option<SystemTime>)> {
    let mut files = BTreeMap::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                files.insert(entry.path(), (metadata.len(), metadata.modified().ok()));
            }
        }
    }
    files
}

fn changed_files(
    directory: &Path,
    before: &BTreeMap<PathBuf, (u64, Option<SystemTime>)>,
) -> Vec<PythonOutputFile> {
    snapshot(directory)
        .into_iter()
        .filter(|(path, state)| before.get(path) != Some(state))
        .map(|(path, (bytes, _))| PythonOutputFile {
            path: path.display().to_string(),
            bytes,
        })
        .collect()
}

struct BoundedOutput {
    head: Vec<String>,
    tail: VecDeque<String>,
    tail_bytes: usize,
    omitted: usize,
}

impl BoundedOutput {
    fn new() -> Self {
        Self {
            head: Vec::new(),
            tail: VecDeque::new(),
            tail_bytes: 0,
            omitted: 0,
        }
    }

    fn push(&mut self, line: String) {
        if self.head.len() < HEAD_LINES {
            self.head.push(line);
            return;
        }
        self.tail_bytes += line.len();
        self.tail.push_back(line);
        while self.tail.len() > TAIL_LINES || (self.tail_bytes > TAIL_BYTES && self.tail.len() > 1)
        {
            if let Some(dropped) = self.tail.pop_front() {
                self.tail_bytes -= dropped.len();
                self.omitted += 1;
            }
        }
    }

    fn finish(self) -> (String, usize) {
        let mut text: String = self.head.concat();
        if self.omitted > 0 {
            text.push_str(&format!("\n… {} lines omitted …\n\n", self.omitted));
        }
        text.extend(self.tail);
        (text, self.omitted)
    }
}

fn run_process(
    program: impl AsRef<std::ffi::OsStr>,
    arguments: &[OsString],
    directory: &Path,
    environment: &[(&str, OsString)],
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<PythonRun, PythonFailure> {
    let mut launcher = Launcher::default()
        .in_dir(directory)
        // An inherited virtual environment would win over the project's own.
        .without_env(["VIRTUAL_ENV", "PYTHONHOME", "PYTHONPATH"]);
    for (name, value) in environment {
        launcher = launcher.with_env(*name, value.clone());
    }
    let mut process = launcher
        .spawn(
            program,
            arguments,
            Spawn {
                stdin: None,
                merge_stderr: true,
            },
        )
        .map_err(|error| {
            tracing::warn!(%error, "Python process could not be started");
            PythonFailure::Failed
        })?;
    let mut output = BoundedOutput::new();
    let ended = process
        .run(Some(Instant::now() + timeout), None, |line| {
            output.push(match line {
                OutputLine::Out(text) | OutputLine::Err(text) => text,
            });
            !cancel.is_cancelled()
        })
        .map_err(|_| PythonFailure::Failed)?;
    let (text, omitted_lines) = output.finish();
    Ok(PythonRun {
        exit_code: match ended {
            Ended::Exited(status) => status.code(),
            _ => None,
        },
        timed_out: ended == Ended::TimedOut,
        output: text,
        omitted_lines,
        output_files: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> PathBuf {
        let directory = std::env::temp_dir().join(format!("yss-python-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn long_output_keeps_its_beginning_and_end() {
        let mut output = BoundedOutput::new();
        for index in 0..1_000 {
            output.push(format!("line {index}\n"));
        }
        let (text, omitted) = output.finish();
        assert_eq!(omitted, 1_000 - HEAD_LINES - TAIL_LINES);
        assert!(text.starts_with("line 0\n"));
        assert!(text.ends_with("line 999\n"));
        assert!(text.contains("lines omitted"));
    }

    #[test]
    fn requirements_replace_earlier_specifiers_of_a_distribution() {
        let directory = project();
        record_requirements(&directory, &["pandas".into(), "Requests>=2".into()]).unwrap();
        let lines = record_requirements(&directory, &["pandas==2.2.3".into()]).unwrap();
        assert_eq!(lines, ["pandas==2.2.3", "Requests>=2"]);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn scripts_round_trip_and_missing_ones_are_reported() {
        let directory = project();
        let runtime = PythonRuntime::new(directory.join("environments"));
        runtime
            .write_script(&directory, "collect", "print(1)\n")
            .unwrap();
        assert_eq!(
            runtime
                .read_script(&directory, "collect.py")
                .unwrap()
                .content,
            "print(1)\n"
        );
        assert_eq!(
            runtime.read_script(&directory, "absent").unwrap_err(),
            PythonFailure::ScriptNotFound
        );
        assert_eq!(
            list_scripts(&scripts_directory(&directory)).unwrap(),
            ["collect.py"]
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// Needs uv or Python 3 on PATH; creates a real environment.
    #[test]
    #[ignore = "creates a Python environment"]
    fn scripts_run_in_the_project_environment_and_report_their_files() {
        let directory = project();
        let runtime = PythonRuntime::new(directory.join("environments"));
        runtime
            .write_script(
                &directory,
                "export",
                "import os, sys\nopen(os.path.join(os.environ['YSSBI_OUTPUT_DIR'], 'a.csv'), 'w').write('x\\n1\\n')\nprint('done', sys.argv[1])\n",
            )
            .unwrap();
        let run = runtime
            .run_script(
                &directory,
                &PythonRunScriptRequest {
                    name: "export".into(),
                    arguments: vec!["now".into()],
                    timeout_seconds: 120,
                },
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(run.exit_code, Some(0), "{}", run.output);
        assert_eq!(run.output.trim(), "done now");
        assert_eq!(run.output_files.len(), 1);
        assert!(run.output_files[0].path.ends_with("a.csv"));
        let timed_out = {
            runtime
                .write_script(&directory, "slow", "import time\ntime.sleep(30)\n")
                .unwrap();
            runtime
                .run_script(
                    &directory,
                    &PythonRunScriptRequest {
                        name: "slow".into(),
                        arguments: Vec::new(),
                        timeout_seconds: 1,
                    },
                    &CancellationToken::default(),
                )
                .unwrap()
        };
        assert!(timed_out.timed_out);
        assert_eq!(timed_out.exit_code, None);
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// Needs uv or Python 3 on PATH and network access to the package index.
    #[test]
    #[ignore = "installs a package from the network"]
    fn packages_install_into_the_project_environment_and_are_recorded() {
        let directory = project();
        let runtime = PythonRuntime::new(directory.join("environments"));
        let installed = runtime
            .install_packages(&directory, &["six".into()], &CancellationToken::default())
            .unwrap();
        assert_eq!(installed.requirements, ["six"]);
        let environment = runtime.inspect(&directory).unwrap();
        assert!(
            environment
                .packages
                .iter()
                .any(|package| package.name == "six")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
