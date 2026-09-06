use dst_ksh_analyze_lib::core::{build_ksh, parse_ksh};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const VERTEX_SOURCE: &str =
    "uniform mat4 ModelViewProj; void main() { gl_Position = ModelViewProj * vec4(1.0); }\n";
const PIXEL_SOURCE: &str = "uniform vec4 Color; void main() { gl_FragColor = Color; }\n";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "dst-ksh-cli-test-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn write_sources(&self, directory: &Path, vs_name: &str, ps_name: &str) {
        fs::write(directory.join(vs_name), VERTEX_SOURCE).unwrap();
        fs::write(directory.join(ps_name), PIXEL_SOURCE).unwrap();
    }

    fn write_ksh(&self) {
        let bytes = build_ksh(
            "fixture",
            "vertex.vs",
            VERTEX_SOURCE,
            "pixel.ps",
            PIXEL_SOURCE,
        )
        .unwrap();
        fs::write(self.path("input.ksh"), bytes).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dst-ksh-analyze-cli"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .expect("failed to launch the CLI binary")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "CLI failed with {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "CLI unexpectedly succeeded\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ksh_unpack_uses_the_second_positional_argument_as_output_directory() {
    let fixture = TestDirectory::new();
    fixture.write_ksh();

    assert_success(&fixture.run(&["input.ksh", "chosen-output"]));

    assert_eq!(
        fs::read_to_string(fixture.path("chosen-output/vertex.vs")).unwrap(),
        VERTEX_SOURCE
    );
    assert_eq!(
        fs::read_to_string(fixture.path("chosen-output/pixel.ps")).unwrap(),
        PIXEL_SOURCE
    );
    assert!(!fixture.path("input").exists());
}

#[test]
fn reversed_uppercase_stage_extensions_are_routed_to_the_correct_stages() {
    let fixture = TestDirectory::new();
    fixture.write_sources(&fixture.0, "vertex.VS", "pixel.PS");

    assert_success(&fixture.run(&["pixel.PS", "vertex.VS", "result.KSH"]));

    let ksh = parse_ksh(&fs::read(fixture.path("result.KSH")).unwrap()).unwrap();
    assert_eq!(ksh.vertex.source_name, "vertex.VS");
    assert_eq!(ksh.vertex.source, VERTEX_SOURCE);
    assert_eq!(ksh.pixel.source_name, "pixel.PS");
    assert_eq!(ksh.pixel.source, PIXEL_SOURCE);
}

#[test]
fn building_refuses_to_replace_an_existing_output_without_force() {
    let fixture = TestDirectory::new();
    fixture.write_sources(&fixture.0, "vertex.vs", "pixel.ps");
    fs::write(fixture.path("result.ksh"), b"existing output").unwrap();

    assert_failure(&fixture.run(&["vertex.vs", "pixel.ps", "result.ksh"]));
    assert_eq!(
        fs::read(fixture.path("result.ksh")).unwrap(),
        b"existing output"
    );

    assert_success(&fixture.run(&["vertex.vs", "pixel.ps", "result.ksh", "--force"]));
    let ksh = parse_ksh(&fs::read(fixture.path("result.ksh")).unwrap()).unwrap();
    assert_eq!(ksh.vertex.source, VERTEX_SOURCE);
    assert_eq!(ksh.pixel.source, PIXEL_SOURCE);
}

#[test]
fn unpacking_refuses_to_replace_existing_stage_files_without_force() {
    let fixture = TestDirectory::new();
    fixture.write_ksh();
    fs::create_dir(fixture.path("output")).unwrap();
    // A collision on the second file must not leave the first file half-extracted.
    fs::write(fixture.path("output/pixel.ps"), b"existing pixel source").unwrap();

    assert_failure(&fixture.run(&["input.ksh", "output"]));
    assert!(!fixture.path("output/vertex.vs").exists());
    assert_eq!(
        fs::read(fixture.path("output/pixel.ps")).unwrap(),
        b"existing pixel source"
    );

    assert_success(&fixture.run(&["input.ksh", "output", "--force"]));
    assert_eq!(
        fs::read_to_string(fixture.path("output/vertex.vs")).unwrap(),
        VERTEX_SOURCE
    );
    assert_eq!(
        fs::read_to_string(fixture.path("output/pixel.ps")).unwrap(),
        PIXEL_SOURCE
    );
}

#[test]
fn no_arguments_returns_failure() {
    let fixture = TestDirectory::new();
    assert_failure(&fixture.run(&[]));
}

#[test]
fn ksh_unpack_rejects_an_extra_third_positional_argument() {
    let fixture = TestDirectory::new();
    fixture.write_ksh();

    assert_failure(&fixture.run(&["input.ksh", "output", "unexpected"]));

    assert!(!fixture.path("output").exists());
    assert!(!fixture.path("unexpected").exists());
}

#[test]
fn directory_build_rejects_an_extra_third_positional_argument() {
    let fixture = TestDirectory::new();
    fs::create_dir(fixture.path("sources")).unwrap();
    fixture.write_sources(&fixture.path("sources"), "vertex.vs", "pixel.ps");

    assert_failure(&fixture.run(&["sources", "result.ksh", "unexpected.ksh"]));

    assert!(!fixture.path("result.ksh").exists());
    assert!(!fixture.path("unexpected.ksh").exists());
}

#[test]
fn version_matches_the_cargo_package_version() {
    let fixture = TestDirectory::new();
    let output = fixture.run(&["--version"]);

    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("dst-ksh-analyze-cli {}", env!("CARGO_PKG_VERSION"))
    );
}
