use std::{
   fmt::Write as _,
   fs,
   io::Write as _,
   process,
};

use cab::{
   report::{
      self,
      Contextful as _,
   },
   syntax,
};
use clap::Parser as _;
use ust::{
   style::StyledExt as _,
   terminal,
   write,
};
use which::which;

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
async fn main() -> report::Termination {
   let cli = Cli::parse();

   let err = &mut terminal::stderr();

   match cli.command {
      Command::Check {
         fail_fast,
         command: Check::Syntax { overwrite },
      } => {
         let mut fail_count: usize = 0;

         let diff_tool = which("difft")
            .or_else(|_| which("diff"))
            .context("failed to find diff tool")?;

         let parse_oracle = syntax::parse_oracle();

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
               let source = fs::read_to_string(&source_file).with_context(|| {
                  format!(
                     "failed to read source file {source_file}",
                     source_file = source_file.display(),
                  )
               })?;

               let expected_display =
                  fs::read_to_string(&expected_display_file).with_context(|| {
                     format!(
                        "failed to read expected display file {expected_display_file}",
                        expected_display_file = expected_display_file.display(),
                     )
                  })?;

               let actual_display = {
                  let node = parse_oracle.parse(syntax::tokenize(&source)).node;
                  format!("{node:#?}")
               };

               let name = source_file.file_stem().unwrap().to_str().unwrap().bold();

               if expected_display == actual_display {
                  write!(err, "expected and actual display matched for ")
                     .context("failed to write to stderr")?;
                  write(err, &name.green()).context("failed to write to stderr")?;
                  return Ok(());
               }

               write!(err, "behaviour has changed for ").context("failed to write to stderr")?;
               write(err, &name.yellow()).context("failed to write to stderr")?;
               write!(err, "! diffing expected vs actual display")
                  .context("failed to write to stderr")?;

               let mut child = process::Command::new(&diff_tool)
                  .arg(&expected_display_file)
                  .arg("/dev/stdin")
                  .stdin(process::Stdio::piped())
                  .spawn()
                  .context("failed to spawn diff tool")?;

               if let Some(mut stdin) = child.stdin.take() {
                  write!(stdin, "{actual_display}")
                     .context("failed to feed display to diff tool")?;
               }

               child
                  .wait()
                  .context("failed to wait for diff tool to complete")?;

               if overwrite {
                  eprintln!("overwriting old test case...");
                  fs::write(&expected_display_file, &actual_display).with_context(|| {
                     format!(
                        "failed to override expected display file {expected_display_file} with \
                         actual display",
                        expected_display_file = expected_display_file.display(),
                     )
                  })?;
               }

               fail_count += 1;

               if fail_fast {
                  report::bail!("failed fast");
               }

               Ok::<(), report::Error>(())
            })?;

         if fail_count > 0 {
            if !fail_fast {
               eprintln!("behaviour has changed for {fail_count} test cases");
            }

            report::bail!("exiting due to {fail_count} previous errors");
         }
      },
   }

   report::Termination::success()
}
