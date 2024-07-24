use std::{
    error,
    fs,
    io::Write as _,
    process,
};

use cab::syntax::parse;
use clap::{
    Parser,
    Subcommand,
};
use clap_verbosity_flag::{
    InfoLevel,
    Verbosity,
};
use log::Level;
use which::which;
use yansi::{
    Condition,
    Paint,
};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity<InfoLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    Check {
        #[command(subcommand)]
        command: Check,

        /// Whether to immediately exit after the first failure.
        #[arg(short, long, global = true)]
        fail_fast: bool,
    },
}

/// Checks the specified crate for correctness.
#[derive(Subcommand, Debug, Clone)]
enum Check {
    /// Compares the test data and expected results with the actual results.
    Syntax,
}

fn real_main() -> Result<(), Box<dyn error::Error>> {
    let cli = Cli::parse();

    yansi::whenever(Condition::TTY_AND_COLOR);

    // Trying to imitate clap to get a consistent experience.
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .format(|buffer, record| {
            let level = match record.level() {
                Level::Error => "error:".red().bold(),
                Level::Warn => "warn:".yellow().bold(),
                Level::Info => "info:".green().bold(),
                Level::Debug => "debug:".blue().bold(),
                Level::Trace => "trace:".cyan().bold(),
            };

            writeln!(buffer, "{level} {arguments}", arguments = record.args())
        })
        .init();

    match cli.command {
        Command::Check {
            command: Check::Syntax,
            fail_fast,
        } => {
            let test_data = fs::read_dir("cab-syntax/test/data")?.filter_map(|entry| {
                let mut path = entry.ok()?.path();

                if !path.extension().is_some_and(|extension| extension == "cab") {
                    return None;
                }

                Some((path.clone(), {
                    path.set_extension("expect");
                    path
                }))
            });

            let mut failed = false;

            let diff_tool = which("difft").or_else(|_| which("diff"))?;

            for (data_file, expected_syntax_file) in test_data {
                let data = fs::read_to_string(&data_file)?;
                let expected_syntax = fs::read_to_string(&expected_syntax_file)?;

                let syntax = {
                    let node = parse(data.as_ref()).syntax();
                    format!("{node:#?}")
                };

                let name = data_file.file_stem().unwrap().to_str().unwrap().bold();

                failed = expected_syntax != syntax;
                if failed {
                    log::warn!(
                        "behaviour has changed for {name}! diffing expected vs. actual syntax:",
                        name = name.red()
                    );

                    let mut child = process::Command::new(&diff_tool)
                        .arg(expected_syntax_file)
                        .arg("/dev/stdin")
                        .stdin(process::Stdio::piped())
                        .spawn()?;

                    if let Some(mut stdin) = child.stdin.take() {
                        write!(stdin, "{syntax}")?;
                    }

                    if fail_fast {
                        break;
                    }
                } else {
                    log::info!(
                        "expected syntax and real syntax matched for {name}",
                        name = name.green()
                    );
                }
            }

            if failed {
                process::exit(1);
            }
        },
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    real_main().unwrap_or_else(|error| {
        log::error!("{error}");
        process::exit(1);
    })
}
