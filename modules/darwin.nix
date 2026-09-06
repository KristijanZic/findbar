self: { config, lib, pkgs, ... }:

let
  cfg = config.programs.findbar;
  inherit (lib) mkEnableOption mkOption types mkIf;

  configJson = pkgs.writeText "findbar-config.json" (builtins.toJSON {
    items = cfg.items;
    keep_unmanaged = cfg.keepUnmanaged;
  });
in
{
  options.programs.findbar = {
    enable = mkEnableOption "findbar declarative Finder sidebar management";

    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The findbar package to use.";
    };

    user = mkOption {
      type = types.nullOr types.str;
      default = config.system.primaryUser or null;
      description = "The macOS user account to configure Finder sidebar favorites for. Defaults to system.primaryUser if defined, or automatically detects the active console/sudo user.";
      example = "myuser";
    };

    keepUnmanaged = mkOption {
      type = types.bool;
      default = false;
      description = "Whether to retain items in the sidebar that are not defined in the Nix configuration.";
    };

    items = mkOption {
      type = types.listOf (
        types.submodule {
          options = {
            name = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "Custom display name in Finder sidebar (defaults to folder name).";
              example = "Downloads";
            };

            path = mkOption {
              type = types.str;
              description = "Path to the folder or file (e.g. ~/Downloads, /Applications).";
              example = "~/Downloads";
            };
          };
        }
      );
      default = [ ];
      description = "List of sidebar items in desired display order.";
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    system.activationScripts.findbar.text = ''
      echo "Configuring Finder sidebar with findbar..."
      TARGET_USER="${if cfg.user != null then cfg.user else ""}"
      if [ -z "$TARGET_USER" ]; then
        if [ -n "$SUDO_USER" ] && [ "$SUDO_USER" != "root" ]; then
          TARGET_USER="$SUDO_USER"
        else
          TARGET_USER="$(/usr/bin/stat -f '%Su' /dev/console 2>/dev/null || true)"
        fi
      fi

      if [ -n "$TARGET_USER" ] && [ "$TARGET_USER" != "root" ]; then
        TARGET_UID="$(id -u "$TARGET_USER" 2>/dev/null || true)"
        if [ -n "$TARGET_UID" ]; then
          echo "findbar: syncing Finder sidebar favorites for user '$TARGET_USER' (UID $TARGET_UID)..."
          /bin/launchctl asuser "$TARGET_UID" sudo -u "$TARGET_USER" -H ${cfg.package}/bin/findbar sync "${configJson}"
        else
          echo "findbar: unable to determine UID for user '$TARGET_USER'. Skipping."
        fi
      else
        echo "findbar: unable to determine target user for Finder sidebar sync. Skipping."
      fi
    '';
  };
}
