self: { config, lib, pkgs, ... }:

let
  cfg = config.programs.findbar;
  inherit (lib) mkEnableOption mkOption types mkIf hm;

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

    keepUnmanaged = mkOption {
      type = lib.types.bool;
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
    home.packages = [ cfg.package ];

    home.activation.findbar = hm.dag.entryAfter [ "writeBoundary" ] ''
      $DRY_RUN_CMD ${cfg.package}/bin/findbar sync "${configJson}"
    '';
  };
}
