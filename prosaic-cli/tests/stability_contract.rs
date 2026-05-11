use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn prosaic() -> Command {
    Command::new(env!("CARGO_BIN_EXE_prosaic"))
}

fn run(args: &[&str], stdin: Option<&str>) -> Output {
    let mut command = prosaic();
    command.args(args);
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn prosaic");
    if let Some(input) = stdin {
        let mut child_stdin = child.stdin.take().expect("child stdin");
        child_stdin
            .write_all(input.as_bytes())
            .expect("write stdin");
    }

    child.wait_with_output().expect("wait for prosaic")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf8")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn unique_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "prosaic-cli-contract-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn path_arg(path: &Path) -> &str {
    path.to_str().expect("test paths are valid utf8")
}

#[test]
fn help_lists_the_stable_cli_contract() {
    let output = run(&["--help"], None);
    assert_success(&output);
    let text = stdout(&output);

    for expected in [
        "prosaic [OPTIONS] < events.jsonl",
        "--preset <name>",
        "--vocab <list>",
        "--strategy <mode>",
        "--smart-quotes",
        "--max-length <N>",
        "--style <name>",
        "--explain",
        "--strict | --lenient | --silent",
        "new <name>",
        "build <project>",
        "test <project>",
    ] {
        assert!(
            text.contains(expected),
            "help output missing stable contract fragment `{expected}`:\n{text}"
        );
    }
}

#[test]
fn json_lines_render_contract_is_stable() {
    let input = "{\"key\":\"code.renamed\",\"entity_type\":\"class\",\"old_name\":\"Foo\",\"new_name\":\"Bar\",\"consumer_count\":3}\n";

    let output = run(
        &["--vocab", "code", "--strategy", "sequential"],
        Some(input),
    );
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "The class Foo was renamed to Bar, which impacts 3 direct consumers.\n"
    );
}

#[test]
fn explain_mode_emits_render_explanation_json() {
    let input = "{\"key\":\"code.renamed\",\"entity_type\":\"class\",\"old_name\":\"Foo\",\"new_name\":\"Bar\",\"consumer_count\":3}\n";

    let output = run(
        &["--vocab", "code", "--strategy", "sequential", "--explain"],
        Some(input),
    );
    assert_success(&output);

    let value: serde_json::Value = serde_json::from_str(stdout(&output).trim()).unwrap();
    assert_eq!(
        value["output"],
        "The class Foo was renamed to Bar, which impacts 3 direct consumers."
    );
    assert_eq!(value["template_key"], "code.renamed");
    assert_eq!(value["variant_index"], 0);
    assert_eq!(value["salience"], "Medium");
    assert_eq!(value["reference_form"], "Full");
    assert_eq!(value["centering_transition"], "NoCb");
}

#[test]
fn project_subcommands_round_trip_a_starter_project() {
    let root = unique_dir("project");
    let dist = root.join("dist");
    let root_arg = path_arg(&root);
    let dist_arg = path_arg(&dist);

    let new_output = run(
        &["new", "contract", "--starter=changelog", "--at", root_arg],
        None,
    );
    assert_success(&new_output);
    assert!(root.join("prosaic.toml").exists());
    assert!(root.join("templates").join("code.modified.toml").exists());

    let test_output = run(&["test", root_arg], None);
    assert_success(&test_output);
    let test_stdout = stdout(&test_output);
    assert!(test_stdout.contains("PASS  authguard-added"));
    assert!(test_stdout.contains("PASS  sample-changeset"));
    assert!(test_stdout.contains("2/2 passed"));

    let build_output = run(
        &["build", root_arg, "--target=both", "--out", dist_arg],
        None,
    );
    assert_success(&build_output);
    assert!(dist.join("prosaic.bundle.json").exists());
    assert!(dist.join("prosaic_bundle.rs").exists());

    let bundle_json = std::fs::read_to_string(dist.join("prosaic.bundle.json")).unwrap();
    let bundle: serde_json::Value = serde_json::from_str(&bundle_json).unwrap();
    assert_eq!(bundle["schema_version"], 1);
    assert_eq!(bundle["name"], "contract");
    assert_eq!(bundle["language"], "en");

    std::fs::remove_dir_all(root).unwrap();
}
