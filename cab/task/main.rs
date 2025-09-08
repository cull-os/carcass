use std::{
   env,
   fmt::Write as _,
   fs,
   io::Write as _,
   process,
};

use cab::syntax;
use clap::Parser as _;
use cyn::{
   self,
   ResultExt as _,
};
use ust::{
   Write as _,
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
      #[arg(long, global = true)]
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
      #[arg(long, conflicts_with = "fail_fast")]
      overwrite: bool,
   },
}

#[tokio::main]
async fn main() -> cyn::Termination {
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
            .chain_err("failed to find diff tool")?;

         let parse_oracle = syntax::ParseOracle::new();

         let root = env::current_dir().unwrap();
         let root = root.parent().unwrap().join("target").join("cab-noder-fuzz");

         fs::read_dir(&root)
            .chain_err("failed to list cab-syntax/test/data")?
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
               let source = fs::read_to_string(&source_file).chain_err_with(|| {
                  format!(
                     "failed to read source file {source_file}",
                     source_file = source_file.display(),
                  )
               })?;

               let expected_display =
                  fs::read_to_string(&expected_display_file).chain_err_with(|| {
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
                     .chain_err("failed to write to stderr")?;
                  write(err, &name.green()).chain_err("failed to write to stderr")?;
                  return Ok(());
               }

               write!(err, "behaviour has changed for ").chain_err("failed to write to stderr")?;
               write(err, &name.yellow()).chain_err("failed to write to stderr")?;
               write!(err, "! diffing expected vs actual display")
                  .chain_err("failed to write to stderr")?;

               let mut child = process::Command::new(&diff_tool)
                  .arg(&expected_display_file)
                  .arg("/dev/stdin")
                  .stdin(process::Stdio::piped())
                  .spawn()
                  .chain_err("failed to spawn diff tool")?;

               if let Some(mut stdin) = child.stdin.take() {
                  write!(stdin, "{actual_display}")
                     .chain_err("failed to feed display to diff tool")?;
               }

               child
                  .wait()
                  .chain_err("failed to wait for diff tool to complete")?;

               if overwrite {
                  eprintln!("overwriting old test case...");
                  fs::write(&expected_display_file, &actual_display).chain_err_with(|| {
                     format!(
                        "failed to override expected display file {expected_display_file} with \
                         actual display",
                        expected_display_file = expected_display_file.display(),
                     )
                  })?;
               }

               fail_count += 1;

               if fail_fast {
                  cyn::bail!("failed fast");
               }

               Ok::<(), cyn::Chain>(())
            })?;

         if fail_count > 0 {
            if !fail_fast {
               eprintln!("behaviour has changed for {fail_count} test cases");
            }

            cyn::bail!("exiting due to {fail_count} previous errors");
         }
      },
   }

   cyn::Termination::success()
}

#[cfg(test)]
mod tests {
   use clap::CommandFactory as _;

   use super::*;

   #[test]
   fn cli() {
      Cli::command().debug_assert();
   }
}
