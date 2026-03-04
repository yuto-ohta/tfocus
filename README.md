# 🎯 tfocus

> ⚠️ **WARNING**: Resource targeting should be avoided unless absolutely necessary!

## What's this? 🤔

tfocus is a **super interactive** tool for selecting and executing Terraform plan/apply on specific resources.
Think of it as an "emergency tool" - not for everyday use.

![tfocus demo](.github/tfocus_01.gif)

## Features 🌟

- 🔍 Peco-like fuzzy finder for Terraform resources
- ⚡ Lightning-fast resource selection
- 🎨 Colorful TUI (Terminal User Interface)
- 🎹 Vim-like keybindings
- 📁 Recursive file scanning

## Installation 🛠️

install from crates.io

```bash
cargo install tfocus
```

install via [brew](https://brew.sh/)

```bash
brew install tfocus
```

install from github
```bash
cargo install --git https://github.com/nwiizo/tfocus
```

## Usage 🎮

```bash
cd your-terraform-project
tfocus
```

1. 🔍 Launch the fuzzy-search UI
2. ⌨️ Mark one or more resources using vim-like keybindings
3. 🎯 Choose operation (`plan` or `apply`) after selection
4. ✅ For `apply`, confirm directly in Terraform's standard prompt

When you choose `apply`, tfocus runs `terraform apply` once without `-auto-approve`.
Terraform will ask for confirmation in the usual way.

## Operation Selection

After selecting resources, tfocus prompts:

```text
Select operation:
  1) plan
  2) apply
```

- `1) plan`: runs `terraform plan` with selected `-target` options
- `2) apply`: runs `terraform apply` with selected `-target` options and Terraform's own confirmation flow

## Non-Interactive Mode

In `--non-interactive` mode, `--operation` is required.

```bash
tfocus --non-interactive --operation plan
tfocus --non-interactive --operation apply
```

## Keybindings 🎹

- `↑`/`k`: Move up
- `↓`/`j`: Move down
- `/`: Incremental search
- `Space`: Toggle selection
- `Enter`: Confirm selection and execute
- `Esc`/`Ctrl+C`: Cancel

> ℹ️ Multiple selections are only supported when all selected resources are in the same directory.

## ⚠️ Important Warning ⚠️

Using terraform resource targeting comes with significant risks:

1.  Potential disruption of the Terraform resource graph
2. 🎲 Risk of state inconsistencies
3. 🧩 Possible oversight of critical dependencies
4. 🤖 Deviation from standard Terraform workflow

## When to Use 🎯

Only use this tool in specific circumstances:
- 🚑 Emergency troubleshooting
- 🔧 Development debugging
- 🧪 Testing environment verification
- 📊 Impact assessment of large-scale changes

For regular operations, always use full `terraform plan` and `apply`!

## Appropriate Use Cases 🎭

You might consider using tfocus when:
- 🔥 Working with large Terraform codebases where you need to verify specific changes
- 🐌 Full plan execution takes too long during development
- 🔍 Emergency inspection of specific resource states
- 💣 Staged application of changes in complex infrastructure

**Remember!** Standard `terraform plan` and `apply` are the best practices for normal operations.

## Development Status 🚧

This is an experimental tool. Use at your own risk!

## Example 📺

```bash
$ tfocus
QUERY>

[ ]    1 [File]     main.tf
▶[✔]   2 [Module]   vpc
 [✔]   3 [Resource] aws_vpc.main

[↑/k]Up [↓/j]Down [Space]Toggle [Enter]Confirm [Esc/Ctrl+C]Cancel
```

## Contributing 🤝

Issues and PRs are welcome!
Please help make this tool safer and more useful.

## License 📜

MIT

## Final Note 🎬

Think of this tool as a "fire exit" -
It's there when you need it, but you hope you never have to use it! 😅

---
made with 🦀 and ❤️ by nwiizo
