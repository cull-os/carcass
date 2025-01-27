use std::{
    error,
    fs,
    io::Write as _,
    process,
};

use cab::syntax;
use clap::Parser as _;
use clap_verbosity_flag::{
    InfoLevel,
    Verbosity,
};
use which::which;
use yansi::Paint as _;

#[derive(clap::Parser)]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity<InfoLevel>,

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

fn actual_main() -> Result<(), Box<dyn error::Error>> {
    let cli = Cli::parse();

    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    // Trying to imitate clap to get a consistent experience.
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .format(|buffer, record| {
            let level = match record.level() {
                log::Level::Error => "error:".red().bold(),
                log::Level::Warn => "warn:".yellow().bold(),
                log::Level::Info => "info:".green().bold(),
                log::Level::Debug => "debug:".blue().bold(),
                log::Level::Trace => "trace:".cyan().bold(),
            };

            writeln!(buffer, "{level} {arguments}", arguments = record.args())
        })
        .init();

    match cli.command {
        Command::Check {
            fail_fast,
            command: Check::Syntax { overwrite },
        } => {
            let test_data = fs::read_dir("cab-syntax/test/data")?.filter_map(|entry| {
                let mut path = entry.ok()?.path();

                if path.extension().is_none_or(|extension| extension != "cab") {
                    return None;
                }

                Some((path.clone(), {
                    path.set_extension("expect");
                    path
                }))
            });

            let mut fail_count: usize = 0;

            let diff_tool = which("difft").or_else(|_| which("diff"))?;

            for (data_file, expected_syntax_file) in test_data {
                let data = fs::read_to_string(&data_file)?;
                let expected_syntax = fs::read_to_string(&expected_syntax_file)?;

                let actual_syntax = {
                    let node = syntax::parse::<_, syntax::node::Expression>(
                        syntax::tokenize(&data),
                        Default::default(),
                    )
                    .syntax;
                    format!("{node:#?}")
                };

                let name = data_file.file_stem().unwrap().to_str().unwrap().bold();

                if expected_syntax == actual_syntax {
                    log::info!(
                        "expected and actual syntax matched for {name}",
                        name = name.green()
                    );
                    continue;
                }

                log::warn!(
                    "behaviour has changed for {name}! diffing expected vs. actual syntax:",
                    name = name.yellow()
                );

                let mut child = process::Command::new(&diff_tool)
                    .arg(&expected_syntax_file)
                    .arg("/dev/stdin")
                    .stdin(process::Stdio::piped())
                    .spawn()?;

                if let Some(mut stdin) = child.stdin.take() {
                    write!(stdin, "{actual_syntax}")?;
                }

                child.wait()?;

                if overwrite {
                    log::warn!("overwriting old test case...");
                    fs::write(&expected_syntax_file, &actual_syntax)?;
                }

                fail_count += 1;
                if fail_fast {
                    break;
                }
            }

            if fail_count > 0 {
                if !fail_fast {
                    log::error!("behaviour has changed for {fail_count} test cases");
                }

                process::exit(1);
            }
        },
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    actual_main().unwrap_or_else(|error| {
        log::error!("{error}");
        process::exit(1);
    })
}
