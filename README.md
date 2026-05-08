# Shell in Rust

A POSIX-like shell built in Rust as part of the [CodeCrafters "Build Your Own Shell" Challenge](https://app.codecrafters.io/courses/shell/overview).

## Features

- **Builtins**: `echo`, `cd`, `pwd`, `type`, `exit`, `history`, `jobs`, `complete`, `declare`
- **External commands**: PATH lookup and `fork`/`execvp` execution
- **Quoting**: single quotes, double quotes, backslash escaping
- **Redirects**: `>`, `>>`, `2>`, `2>>` for stdout/stderr
- **Pipelines**: multi-stage pipes with mixed builtins and externals
- **Background jobs**: `&` operator, `jobs` listing, automatic reaping
- **History**: readline history with `history` builtin, `-r`/`-w`/`-a` flags, `HISTFILE` support
- **Tab completion**: builtin/command/file completions via rustyline, custom completer scripts (`complete -C`), `complete -p`/`-r` management, `COMP_LINE`/`COMP_POINT` env vars
- **Shell variables**: `declare NAME=VALUE`, `declare -p`, identifier validation, `$VAR` and `${VAR}` parameter expansion

## Project Structure

```
src/
  main.rs        — entry point, main loop, module wiring
  parser.rs      — tokenization, quoting, redirects, variable expansion
  exec.rs        — command dispatch, builtins, external execution
  pipeline.rs    — multi-stage pipeline execution
  completion.rs  — rustyline completer, completer scripts
  history.rs     — history file operations
  jobs.rs        — background job management
  declare.rs     — shell variable storage and declare builtin
```

## Dependencies

- [rustyline](https://crates.io/crates/rustyline) — readline with history and tab completion
- [nix](https://crates.io/crates/nix) — safe wrappers for fork/exec/waitpid/pipe
- [anyhow](https://crates.io/crates/anyhow) / [thiserror](https://crates.io/crates/thiserror) — error handling

## Running

```sh
./your_program.sh
```
