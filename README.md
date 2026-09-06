# findbar

**findbar** is a native macOS CLI tool and declarative Nix module (`nix-darwin` and `home-manager`) for managing macOS Finder sidebar Favorites.

Written in Rust using native `CoreServices` (`LSSharedFileList`) system APIs.

---

## Features

- **Blazing Fast & Memory Safe**: Sound Rust RAII wrappers around `CoreServices.framework`.
- **Non-Destructive Synchronization**: Smart diff reconciliation preserves existing items, adjusts order, and avoids sidebar flickering.
- **Localized Display Names**: Omitting a custom name lets macOS automatically resolve localized system folder names (*Téléchargements*, *Descargas*, etc.).
- **Duplicate Prevention**: Safeguards against creating duplicate entries in the sidebar.
- **Nix Modules**: First-class declarative modules for both **nix-darwin** and **Home Manager**.
- **Shell Completions**: Built-in tab-completion generation for Bash, Zsh, and Fish.
- **CLI Utilities**:
  - `list`: View sidebar favorites in tabular format, JSON (`--json`), or as a Nix snippet (`--format nix`).
  - `add`: Add folders/files with custom display names, position controls (`--before`, `--after`, `--beginning`), and `--force` for duplicates.
  - `remove`: Remove items by name, path, or clear all (`--all`).
  - `sync`: Synchronize sidebar state declaratively against a JSON/TOML file or stdin.
  - `export`: Dump your current Finder favorites directly into Nix, TOML, or JSON format.
  - `completions`: Generate shell completions for Bash, Zsh, Fish, PowerShell, or Elvish.

---

## CLI Usage

### Listing Favorites
```bash
# Formatted colored table
findbar list

# JSON format
findbar list --json

# Direct Nix representation
findbar list --format nix
```

### Imperative Modifications
```bash
# Add an item to the end (duplicate-safe; localized folder name resolved automatically)
findbar add ~/Projects

# Add with a custom display name
findbar add ~/Developer --name "Code"

# Allow adding a duplicate item
findbar add ~/Projects --force

# Insert before, after, or at the beginning
findbar add ~/Music --before "Downloads"
findbar add ~/Pictures --after "Desktop"
findbar add ~/Desktop --beginning

# Remove an item by name or path
findbar remove "Music"
findbar remove ~/Pictures

# Remove all items
findbar remove --all --force
```

### Declarative Sync
```bash
# Synchronize sidebar to match config.json exactly (removes unmanaged items)
findbar sync config.json

# Synchronize using TOML
findbar sync config.toml

# Keep unmanaged manual items
findbar sync config.json --keep-unmanaged

# Preview planned modifications without touching Finder
findbar sync config.json --dry-run

# Pipe declarative JSON directly via stdin
echo '[{"path": "/Applications"}, {"path": "~/Downloads"}]' | findbar sync -
```

### Exporting Configurations
```bash
# Export as a ready-to-paste Nix configuration
findbar export --format nix

# Export as JSON or TOML
findbar export --format toml > config.toml
findbar export --format json > config.json
```

### Shell Completions
```bash
# Zsh (e.g. into a completions directory)
findbar completions zsh > ~/.zsh/completion/_findbar

# Fish
findbar completions fish > ~/.config/fish/completions/findbar.fish

# Bash
findbar completions bash > ~/.local/share/bash-completion/completions/findbar
```

---

## Nix Integration

### 1. nix-darwin

Add `findbar` to your `flake.nix` inputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nix-darwin.url = "github:LnL7/nix-darwin";
    nix-darwin.inputs.nixpkgs.follows = "nixpkgs";

    findbar.url = "github:aresminos/findbar";
    findbar.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nix-darwin, findbar, ... }: {
    darwinConfigurations."my-mac" = nix-darwin.lib.darwinSystem {
      modules = [
        findbar.darwinModules.default
        {
          programs.findbar = {
            enable = true;
            # user = "myuser"; # Optional: defaults to system.primaryUser or the active user
            # keepUnmanaged = false; # Set to true to retain manual unmanaged items
            items = [
              # Omitting 'name' allows macOS to localize system folders automatically
              { path = "/Applications"; }
              { path = "~/Downloads"; }
              { path = "~/Documents"; }
              { name = "Workspace"; path = "~/Projects"; }
            ];
          };
        }
      ];
    };
  };
}
```

### 2. Home Manager

```nix
{
  inputs = {
    home-manager.url = "github:nix-community/home-manager";
    findbar.url = "github:aresminos/findbar";
  };

  outputs = { self, home-manager, findbar, ... }: {
    homeConfigurations."myuser" = home-manager.lib.homeManagerConfiguration {
      modules = [
        findbar.homeManagerModules.default
        {
          programs.findbar = {
            enable = true;
            items = [
              { path = "/Applications"; }
              { path = "~/Downloads"; }
              { name = "Workspace"; path = "~/Projects"; }
            ];
          };
        }
      ];
    };
  };
}
```

---

## Development

```bash
# Enter dev shell with Rust toolchain
nix develop

# Build and run tests
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Check flake
nix flake check
```

## License

GPL-3.0-or-later
