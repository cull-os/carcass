use std::{
    fs,
    io::Write as _,
    process,
};

use cab::{
    syntax,
    why::{
        self,
        Contextful as _,
    },
};
use clap::Parser as _;
use which::which;
use yansi::Paint as _;

#[derive(clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Command {
    Check {
        /// Whether to immediately exit after the first failure.
        #[arg(short, long, global = true)]
        fail_fast: bool,

        #[command(subcommand)]
        command: Check,
    },
}

/// Checks the specified crate for correctness.
#[derive(clap::Subcommand, Debug, Clone)]
enum Check {
    /// Compares the test data and expected results with the actual results.
    Syntax {
        /// Whether to overwrite test cases that do not match with the actual
        /// result.
        #[arg(short, long, conflicts_with = "fail_fast")]
        overwrite: bool,
    },
}

#[tokio::main]
async fn main() -> why::Termination {
    let cli = Cli::parse();

    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    match cli.command {
        Command::Check {
            fail_fast,
            command: Check::Syntax { overwrite },
        } => {
            let mut fail_count: usize = 0;

            let diff_tool = which("difft")
                .or_else(|_| which("diff"))
                .context("failed to find diff tool")?;

            let oracle = syntax::oracle();

            fs::read_dir("cab-syntax/test/data")
                .context("failed to list cab-syntax/test/data")?
                .filter_map(|entry| {
                    let mut path = entry.ok()?.path();

                    if path.extension().is_none_or(|extension| extension != "cab") {
                        return None;
                    }

                    Some((path.clone(), {
                        path.set_extension("expect");
                        path
                    }))
                })
                .try_for_each(|(source_file, expected_display_file)| {
                    let source = fs::read_to_string(&source_file)
                        .with_context(|| format!("failed to read source file {source_file:?}"))?;

                    let expected_display = fs::read_to_string(&expected_display_file)
                        .with_context(|| format!("failed to read expected display file {expected_display_file:?}"))?;

                    let actual_display = {
                        let node = oracle.parse(syntax::tokenize(&source)).node;
                        format!("{node:#?}")
                    };

                    let name = source_file.file_stem().unwrap().to_str().unwrap().bold();

                    if expected_display == actual_display {
                        eprintln!("expected and actual display matched for {name}", name = name.green());
                        return Ok(());
                    }

                    eprintln!(
                        "behaviour has changed for {name}! diffing expected vs actual display",
                        name = name.yellow()
                    );

                    let mut child = process::Command::new(&diff_tool)
                        .arg(&expected_display_file)
                        .arg("/dev/stdin")
                        .stdin(process::Stdio::piped())
                        .spawn()
                        .context("failed to spawn diff tool")?;

                    if let Some(mut stdin) = child.stdin.take() {
                        write!(stdin, "{actual_display}").context("failed to feed display to diff tool")?;
                    }

                    child.wait().context("failed to wait for diff tool to complete")?;

                    if overwrite {
                        eprintln!("overwriting old test case...");
                        fs::write(&expected_display_file, &actual_display).with_context(|| {
                            format!(
                                "failed to override expected display file {expected_display_file:?} with actual \
                                 display"
                            )
                        })?;
                    }

                    fail_count += 1;

                    if fail_fast {
                        why::bail!("failed fast");
                    }

                    Ok(())
                })?;

            if fail_count > 0 {
                if !fail_fast {
                    eprintln!("behaviour has changed for {fail_count} test cases");
                }

                process::exit(1);
            }
        },
    }

    why::Termination::success()
}
